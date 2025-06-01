use std::collections::VecDeque;

#[derive(Debug, Clone)]
pub struct PriceWindow {
    pub prices: VecDeque<f64>,
    pub max_size: usize,
}

impl PriceWindow {
    pub fn new(size: usize) -> Self {
        Self {
            prices: VecDeque::with_capacity(size + 1),
            max_size: size,
        }
    }

    pub fn add_price(&mut self, price: f64) {
        self.prices.push_back(price);
        if self.prices.len() > self.max_size {
            self.prices.pop_front();
        }
    }

    pub fn is_ready(&self) -> bool {
        self.prices.len() >= self.max_size
    }

    pub fn calculate_rsi(&self, period: usize) -> Option<f64> {
        if self.prices.len() < period + 1 {
            return None;
        }

        let mut gains = Vec::new();
        let mut losses = Vec::new();

        // Calculate price changes for the last 'period' periods
        for i in (self.prices.len() - period)..self.prices.len() {
            if i == 0 { continue; }
            
            let change = self.prices[i] - self.prices[i - 1];
            if change > 0.0 {
                gains.push(change);
                losses.push(0.0);
            } else {
                gains.push(0.0);
                losses.push(-change);
            }
        }

        if gains.is_empty() || losses.is_empty() {
            return None;
        }

        let avg_gain: f64 = gains.iter().sum::<f64>() / gains.len() as f64;
        let avg_loss: f64 = losses.iter().sum::<f64>() / losses.len() as f64;

        if avg_loss == 0.0 {
            return Some(100.0); // RSI = 100 when there are no losses
        }

        let rs = avg_gain / avg_loss;
        let rsi = 100.0 - (100.0 / (1.0 + rs));

        Some(rsi)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rsi_calculation() {
        let mut window = PriceWindow::new(15); // Need 15 prices for 14-period RSI
        
        // Sample price data (upward trend should give lower RSI than 50)
        let prices = vec![
            44.0, 44.25, 44.5, 43.75, 44.5, 44.0, 44.25,
            44.75, 45.0, 45.25, 45.5, 45.75, 46.0, 46.25, 46.5
        ];
        
        for price in prices {
            window.add_price(price);
        }
        
        let rsi = window.calculate_rsi(14);
        assert!(rsi.is_some());
        let rsi_value = rsi.unwrap();
        
        // RSI should be between 0 and 100
        assert!(rsi_value >= 0.0 && rsi_value <= 100.0);
        
        // For an upward trend, RSI should be > 50
        assert!(rsi_value > 50.0);
        
        println!("RSI for upward trend: {:.2}", rsi_value);
    }

    #[test]
    fn test_rsi_downward_trend() {
        let mut window = PriceWindow::new(15);
        
        // Sample price data (downward trend should give higher RSI than in upward trend)
        let prices = vec![
            46.5, 46.25, 46.0, 45.75, 45.5, 45.25, 45.0,
            44.75, 44.5, 44.25, 44.0, 43.75, 43.5, 43.25, 43.0
        ];
        
        for price in prices {
            window.add_price(price);
        }
        
        let rsi = window.calculate_rsi(14);
        assert!(rsi.is_some());
        let rsi_value = rsi.unwrap();
        
        // RSI should be between 0 and 100
        assert!(rsi_value >= 0.0 && rsi_value <= 100.0);
        
        // For a downward trend, RSI should be < 50
        assert!(rsi_value < 50.0);
        
        println!("RSI for downward trend: {:.2}", rsi_value);
    }

    #[test]
    fn test_rsi_insufficient_data() {
        let mut window = PriceWindow::new(15);
        
        // Only add 10 prices (insufficient for 14-period RSI)
        for i in 1..=10 {
            window.add_price(i as f64);
        }
        
        let rsi = window.calculate_rsi(14);
        assert!(rsi.is_none());
    }

    #[test]
    fn test_rsi_all_gains() {
        let mut window = PriceWindow::new(15);
        
        // Prices that only go up (should give RSI close to 100)
        for i in 1..=15 {
            window.add_price(i as f64);
        }
        
        let rsi = window.calculate_rsi(14);
        assert!(rsi.is_some());
        let rsi_value = rsi.unwrap();
        
        // Should be very high RSI (close to 100) since there are no losses
        assert!(rsi_value > 95.0);
        
        println!("RSI for all gains: {:.2}", rsi_value);
    }

    #[test]
    fn test_window_size_management() {
        let mut window = PriceWindow::new(5);
        
        // Add more prices than window size
        for i in 1..=10 {
            window.add_price(i as f64);
        }
        
        // Window should only keep the last 5 prices
        assert_eq!(window.prices.len(), 5);
        assert_eq!(window.prices[0], 6.0); // First in window should be 6th price added
        assert_eq!(window.prices[4], 10.0); // Last should be 10th price added
    }
}