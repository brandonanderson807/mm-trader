-- Historical data analysis queries for warehouse
-- These can be run through the warehouse REST API /sql endpoint

-- Get hourly OHLC data for a token
-- Replace ? with actual values: token, start_timestamp, end_timestamp
SELECT 
    token,
    DATE_TRUNC('hour', timestamp) as hour,
    FIRST_VALUE(price ORDER BY timestamp) as open,
    MAX(price) as high,
    MIN(price) as low,
    LAST_VALUE(price ORDER BY timestamp) as close,
    COUNT(*) as tick_count
FROM prices 
WHERE token = 'ETH' AND timestamp >= '2024-01-01T00:00:00Z' AND timestamp <= '2024-01-02T00:00:00Z'
GROUP BY token, DATE_TRUNC('hour', timestamp)
ORDER BY token, hour;

-- Calculate RSI for a token
WITH price_changes AS (
    SELECT 
        token,
        timestamp,
        price,
        price - LAG(price, 1) OVER (PARTITION BY token ORDER BY timestamp) as price_change
    FROM prices
    WHERE token = 'BTC' AND timestamp >= '2024-01-01T00:00:00Z' AND timestamp <= '2024-01-02T00:00:00Z'
),
gains_losses AS (
    SELECT 
        token,
        timestamp,
        CASE WHEN price_change > 0 THEN price_change ELSE 0 END as gain,
        CASE WHEN price_change < 0 THEN ABS(price_change) ELSE 0 END as loss
    FROM price_changes
    WHERE price_change IS NOT NULL
),
avg_gains_losses AS (
    SELECT 
        token,
        timestamp,
        AVG(gain) OVER (PARTITION BY token ORDER BY timestamp ROWS 13 PRECEDING) as avg_gain,
        AVG(loss) OVER (PARTITION BY token ORDER BY timestamp ROWS 13 PRECEDING) as avg_loss
    FROM gains_losses
)
SELECT 
    token,
    timestamp,
    CASE 
        WHEN avg_loss = 0 THEN 100
        ELSE 100 - (100 / (1 + (avg_gain / avg_loss)))
    END as rsi
FROM avg_gains_losses
ORDER BY timestamp;

-- Moving averages analysis
SELECT 
    token,
    timestamp,
    price,
    AVG(price) OVER (PARTITION BY token ORDER BY timestamp ROWS 19 PRECEDING) as sma_20,
    AVG(price) OVER (PARTITION BY token ORDER BY timestamp ROWS 49 PRECEDING) as sma_50,
    AVG(price) OVER (PARTITION BY token ORDER BY timestamp ROWS 199 PRECEDING) as sma_200
FROM prices
WHERE token = 'ETH' AND timestamp >= '2024-01-01T00:00:00Z' AND timestamp <= '2024-01-02T00:00:00Z'
ORDER BY timestamp;

-- Price correlation between two tokens
WITH token_prices AS (
    SELECT 
        DATE_TRUNC('hour', timestamp) as hour,
        token,
        AVG(price) as avg_price
    FROM prices
    WHERE timestamp >= '2024-01-01T00:00:00Z' AND timestamp <= '2024-01-02T00:00:00Z'
    GROUP BY DATE_TRUNC('hour', timestamp), token
),
pivot_data AS (
    SELECT 
        hour,
        MAX(CASE WHEN token = 'BTC' THEN avg_price END) as btc_price,
        MAX(CASE WHEN token = 'ETH' THEN avg_price END) as eth_price
    FROM token_prices
    WHERE token IN ('BTC', 'ETH')
    GROUP BY hour
    HAVING COUNT(DISTINCT token) = 2
)
SELECT 
    CORR(btc_price, eth_price) as btc_eth_correlation
FROM pivot_data;

-- Volatility analysis
WITH returns AS (
    SELECT 
        token,
        timestamp,
        price,
        (price - LAG(price, 1) OVER (PARTITION BY token ORDER BY timestamp)) / 
        LAG(price, 1) OVER (PARTITION BY token ORDER BY timestamp) as return
    FROM prices
    WHERE token = 'BTC' AND timestamp >= '2024-01-01T00:00:00Z' AND timestamp <= '2024-01-02T00:00:00Z'
)
SELECT 
    token,
    timestamp,
    price,
    STDDEV(return) OVER (PARTITION BY token ORDER BY timestamp ROWS 19 PRECEDING) as volatility_20,
    STDDEV(return) OVER (PARTITION BY token ORDER BY timestamp ROWS 99 PRECEDING) as volatility_100
FROM returns
ORDER BY timestamp;

-- Support and resistance levels
WITH price_levels AS (
    SELECT 
        token,
        ROUND(price, 2) as price_level,
        COUNT(*) as touch_count,
        MIN(timestamp) as first_touch,
        MAX(timestamp) as last_touch
    FROM prices
    WHERE token = 'ETH' AND timestamp >= '2024-01-01T00:00:00Z' AND timestamp <= '2024-01-02T00:00:00Z'
    GROUP BY token, ROUND(price, 2)
    HAVING COUNT(*) >= 3
)
SELECT 
    token,
    price_level,
    touch_count,
    first_touch,
    last_touch,
    CASE 
        WHEN touch_count >= 5 THEN 'Strong'
        WHEN touch_count >= 3 THEN 'Moderate'
        ELSE 'Weak'
    END as level_strength
FROM price_levels
ORDER BY touch_count DESC, price_level;

-- Price momentum analysis
SELECT 
    token,
    timestamp,
    price,
    (price - LAG(price, 1) OVER (PARTITION BY token ORDER BY timestamp)) / 
    LAG(price, 1) OVER (PARTITION BY token ORDER BY timestamp) * 100 as pct_change_1,
    (price - LAG(price, 24) OVER (PARTITION BY token ORDER BY timestamp)) / 
    LAG(price, 24) OVER (PARTITION BY token ORDER BY timestamp) * 100 as pct_change_24h,
    (price - LAG(price, 168) OVER (PARTITION BY token ORDER BY timestamp)) / 
    LAG(price, 168) OVER (PARTITION BY token ORDER BY timestamp) * 100 as pct_change_7d
FROM prices
WHERE token = 'BTC' AND timestamp >= '2024-01-01T00:00:00Z' AND timestamp <= '2024-01-02T00:00:00Z'
ORDER BY timestamp;