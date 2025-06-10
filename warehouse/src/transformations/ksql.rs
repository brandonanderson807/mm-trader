// KSQL transformation queries for real-time streaming

pub const CREATE_PRICE_STREAM: &str = r#"
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
"#;

pub const CREATE_RSI_STREAM: &str = r#"
CREATE STREAM rsi_stream AS
SELECT 
    token,
    timestamp,
    price,
    COLLECT_LIST(price) OVER (
        PARTITION BY token 
        WINDOW RANGE INTERVAL '14' PRECEDING
    ) AS price_window
FROM price_stream
PARTITION BY token;
"#;

pub const CREATE_MOVING_AVERAGE_STREAM: &str = r#"
CREATE STREAM moving_avg_stream AS
SELECT 
    token,
    timestamp,
    price,
    AVG(price) OVER (
        PARTITION BY token 
        WINDOW RANGE INTERVAL '20' PRECEDING
    ) AS sma_20,
    AVG(price) OVER (
        PARTITION BY token 
        WINDOW RANGE INTERVAL '50' PRECEDING
    ) AS sma_50
FROM price_stream
PARTITION BY token;
"#;

pub const CREATE_PRICE_CHANGES_STREAM: &str = r#"
CREATE STREAM price_changes_stream AS
SELECT 
    token,
    timestamp,
    price,
    LAG(price, 1) OVER (PARTITION BY token) AS prev_price,
    (price - LAG(price, 1) OVER (PARTITION BY token)) / LAG(price, 1) OVER (PARTITION BY token) * 100 AS price_change_pct
FROM price_stream
PARTITION BY token;
"#;

pub const CREATE_VOLATILITY_STREAM: &str = r#"
CREATE STREAM volatility_stream AS
SELECT 
    token,
    timestamp,
    STDDEV(price) OVER (
        PARTITION BY token 
        WINDOW RANGE INTERVAL '20' PRECEDING
    ) AS volatility_20
FROM price_stream
PARTITION BY token;
"#;

pub const CREATE_FEATURE_SINK: &str = r#"
CREATE STREAM feature_sink AS
SELECT 
    p.token,
    p.timestamp,
    p.price,
    ma.sma_20,
    ma.sma_50,
    pc.price_change_pct,
    v.volatility_20
FROM price_stream p
LEFT JOIN moving_avg_stream ma ON p.token = ma.token AND p.timestamp = ma.timestamp
LEFT JOIN price_changes_stream pc ON p.token = pc.token AND p.timestamp = pc.timestamp
LEFT JOIN volatility_stream v ON p.token = v.token AND p.timestamp = v.timestamp
PARTITION BY p.token;
"#;

pub const INSERT_INTO_FEATURES_TOPIC: &str = r#"
INSERT INTO features_topic
SELECT 
    token,
    timestamp,
    price,
    sma_20,
    sma_50,
    price_change_pct,
    volatility_20
FROM feature_sink;
"#;

pub fn get_all_ksql_queries() -> Vec<&'static str> {
    vec![
        CREATE_PRICE_STREAM,
        CREATE_RSI_STREAM,
        CREATE_MOVING_AVERAGE_STREAM,
        CREATE_PRICE_CHANGES_STREAM,
        CREATE_VOLATILITY_STREAM,
        CREATE_FEATURE_SINK,
        INSERT_INTO_FEATURES_TOPIC,
    ]
}