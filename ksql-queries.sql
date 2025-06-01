-- KSQL Queries for Real-time Metrics on Price Data
-- Run these in the ksqlDB CLI to create streaming analytics

-- 1. Create a stream from the price-data topic
CREATE STREAM price_stream (
    token VARCHAR,
    timestamp VARCHAR,
    open DOUBLE,
    high DOUBLE,
    low DOUBLE,
    close DOUBLE,
    volume DOUBLE,
    source VARCHAR
) WITH (
    KAFKA_TOPIC='price-data',
    VALUE_FORMAT='JSON'
);

-- 2. Create a stream with parsed timestamp for time-based operations
CREATE STREAM price_stream_with_time AS
SELECT 
    token,
    STRINGTOTIMESTAMP(timestamp, 'yyyy-MM-dd''T''HH:mm:ss.SSSSSS''Z''') AS price_time,
    open,
    high,
    low,
    close,
    volume,
    source,
    (high - low) AS price_spread,
    ((high - low) / close) * 100 AS volatility_percent
FROM price_stream
EMIT CHANGES;

-- 3. Real-time moving averages (requires windowing)
CREATE TABLE moving_averages AS
SELECT 
    token,
    AVG(close) AS avg_close_5min,
    AVG(volume) AS avg_volume_5min,
    AVG(volatility_percent) AS avg_volatility_5min,
    COUNT(*) AS price_count
FROM price_stream_with_time 
WINDOW TUMBLING (SIZE 5 MINUTES)
GROUP BY token
EMIT CHANGES;

-- 4. Price changes and momentum
CREATE STREAM price_changes AS
SELECT 
    token,
    price_time,
    close,
    LAG(close, 1) OVER (PARTITION BY token) AS prev_close,
    close - LAG(close, 1) OVER (PARTITION BY token) AS price_change,
    ((close - LAG(close, 1) OVER (PARTITION BY token)) / LAG(close, 1) OVER (PARTITION BY token)) * 100 AS price_change_percent
FROM price_stream_with_time
EMIT CHANGES;

-- 5. Create a table for price statistics
CREATE TABLE price_stats AS
SELECT 
    token,
    MIN(close) AS min_price,
    MAX(close) AS max_price,
    AVG(close) AS avg_price,
    MIN(low) AS overall_low,
    MAX(high) AS overall_high,
    SUM(volume) AS total_volume,
    COUNT(*) AS total_records
FROM price_stream_with_time
GROUP BY token
EMIT CHANGES;

-- 6. High volatility alerts (volatility > 5%)
CREATE STREAM high_volatility_alerts AS
SELECT 
    token,
    price_time,
    close,
    volatility_percent,
    'HIGH_VOLATILITY' AS alert_type
FROM price_stream_with_time
WHERE volatility_percent > 5.0
EMIT CHANGES;

-- 7. Volume spikes detection
CREATE STREAM volume_spikes AS
SELECT 
    token,
    price_time,
    volume,
    'VOLUME_SPIKE' AS alert_type
FROM price_stream_with_time
WHERE volume > 500000  -- Adjust threshold as needed
EMIT CHANGES;

-- 8. Create a stream from features topic to analyze RSI
CREATE STREAM features_stream (
    timestamp VARCHAR,
    asset VARCHAR,
    feature_type VARCHAR,
    value DOUBLE,
    metadata MAP<VARCHAR, VARCHAR>
) WITH (
    KAFKA_TOPIC='features',
    VALUE_FORMAT='JSON'
);

-- 9. RSI analysis - Overbought/Oversold conditions
CREATE STREAM rsi_signals AS
SELECT 
    asset,
    STRINGTOTIMESTAMP(timestamp, 'yyyy-MM-dd''T''HH:mm:ss.SSSSSS''Z''') AS signal_time,
    value AS rsi_value,
    CASE 
        WHEN value > 70 THEN 'OVERBOUGHT'
        WHEN value < 30 THEN 'OVERSOLD'
        ELSE 'NEUTRAL'
    END AS rsi_signal
FROM features_stream
WHERE feature_type = 'rsi'
EMIT CHANGES;

-- 10. RSI statistics by asset
CREATE TABLE rsi_stats AS
SELECT 
    asset,
    AVG(value) AS avg_rsi,
    MIN(value) AS min_rsi,
    MAX(value) AS max_rsi,
    COUNT(*) AS rsi_count
FROM features_stream
WHERE feature_type = 'rsi'
GROUP BY asset
EMIT CHANGES;

-- Example queries to run after data starts flowing:

-- Query 1: Show real-time price changes
-- SELECT * FROM price_changes EMIT CHANGES;

-- Query 2: Show current moving averages
-- SELECT * FROM moving_averages;

-- Query 3: Monitor high volatility events
-- SELECT * FROM high_volatility_alerts EMIT CHANGES;

-- Query 4: Watch RSI signals
-- SELECT * FROM rsi_signals EMIT CHANGES;

-- Query 5: Check overall price statistics
-- SELECT * FROM price_stats;

-- Query 6: Show current RSI levels
-- SELECT asset, rsi_value, rsi_signal FROM rsi_signals 
-- WHERE ROWTIME > (UNIX_TIMESTAMP() - 300) * 1000; -- Last 5 minutes