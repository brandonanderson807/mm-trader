-- KSQL setup queries for real-time data transformations
-- Run these queries in KSQL CLI to set up streaming transformations

-- Create stream from price-data topic
CREATE STREAM price_stream (
    token VARCHAR,
    timestamp BIGINT,
    price DOUBLE,
    volume DOUBLE,
    source VARCHAR
) WITH (
    KAFKA_TOPIC='price-data',
    VALUE_FORMAT='JSON'
);

-- Create moving averages stream
CREATE STREAM moving_avg_stream AS
SELECT 
    token,
    timestamp,
    price,
    AVG(price) OVER (
        PARTITION BY token 
        WINDOW TUMBLING (SIZE 20 ROWS)
    ) AS sma_20,
    AVG(price) OVER (
        PARTITION BY token 
        WINDOW TUMBLING (SIZE 50 ROWS)
    ) AS sma_50
FROM price_stream
PARTITION BY token
EMIT CHANGES;

-- Create price change stream
CREATE STREAM price_changes_stream AS
SELECT 
    token,
    timestamp,
    price,
    LAG(price, 1) OVER (PARTITION BY token WINDOW SESSION (60000 MILLISECONDS)) AS prev_price,
    (price - LAG(price, 1) OVER (PARTITION BY token WINDOW SESSION (60000 MILLISECONDS))) / 
    LAG(price, 1) OVER (PARTITION BY token WINDOW SESSION (60000 MILLISECONDS)) * 100 AS price_change_pct
FROM price_stream
PARTITION BY token
EMIT CHANGES;

-- Create volatility stream
CREATE STREAM volatility_stream AS
SELECT 
    token,
    timestamp,
    STDDEV(price) OVER (
        PARTITION BY token 
        WINDOW TUMBLING (SIZE 20 ROWS)
    ) AS volatility_20
FROM price_stream
PARTITION BY token
EMIT CHANGES;

-- Create features table for storing computed features
CREATE TABLE features_table AS
SELECT 
    p.token AS token,
    LATEST_BY_OFFSET(p.timestamp) AS latest_timestamp,
    LATEST_BY_OFFSET(p.price) AS latest_price,
    LATEST_BY_OFFSET(ma.sma_20) AS sma_20,
    LATEST_BY_OFFSET(ma.sma_50) AS sma_50,
    LATEST_BY_OFFSET(pc.price_change_pct) AS price_change_pct,
    LATEST_BY_OFFSET(v.volatility_20) AS volatility_20
FROM price_stream p
LEFT JOIN moving_avg_stream ma WITHIN 1 HOUR ON p.token = ma.token
LEFT JOIN price_changes_stream pc WITHIN 1 HOUR ON p.token = pc.token
LEFT JOIN volatility_stream v WITHIN 1 HOUR ON p.token = v.token
GROUP BY p.token
EMIT CHANGES;

-- Create sink to features topic
CREATE STREAM features_sink AS
SELECT 
    token,
    latest_timestamp,
    latest_price,
    sma_20,
    sma_50,
    price_change_pct,
    volatility_20
FROM features_table
EMIT CHANGES;

-- Insert into features topic
INSERT INTO features_topic
SELECT * FROM features_sink;