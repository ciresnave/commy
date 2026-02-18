// Financial Ticker system configuration and state management

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Configuration for the ticker system
#[derive(Clone, Debug)]
pub struct TickerConfig {
    pub commy_url: String,
    pub commy_tenant: String,
    pub market_update_interval_ms: u64,
    pub max_price_history: usize,
    pub alert_check_interval_ms: u64,
    pub price_precision: u32,
}

impl Default for TickerConfig {
    fn default() -> Self {
        TickerConfig {
            commy_url: "wss://localhost:8443".to_string(),
            commy_tenant: "financial_market".to_string(),
            market_update_interval_ms: 50, // 20 updates/second
            max_price_history: 100000,
            alert_check_interval_ms: 100,
            price_precision: 4,
        }
    }
}

/// Portfolio information for tracking
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Portfolio {
    pub user_id: String,
    pub holdings: HashMap<String, f64>, // symbol -> quantity
    pub cash: f64,
}

impl Portfolio {
    pub fn new(user_id: String, cash: f64) -> Self {
        Portfolio {
            user_id,
            holdings: HashMap::new(),
            cash,
        }
    }

    pub fn buy(&mut self, symbol: String, quantity: f64, price: f64) -> Result<(), String> {
        let cost = quantity * price;
        if cost > self.cash {
            return Err("Insufficient funds".to_string());
        }

        self.cash -= cost;
        *self.holdings.entry(symbol).or_insert(0.0) += quantity;
        Ok(())
    }

    pub fn sell(&mut self, symbol: String, quantity: f64, price: f64) -> Result<(), String> {
        let held = self.holdings.get(&symbol).copied().unwrap_or(0.0);
        if quantity > held {
            return Err("Insufficient shares".to_string());
        }

        self.cash += quantity * price;
        self.holdings.insert(symbol, held - quantity);
        Ok(())
    }

    pub fn get_value(&self, prices: &HashMap<String, f64>) -> f64 {
        let stocks_value: f64 = self
            .holdings
            .iter()
            .map(|(symbol, quantity)| {
                let price = prices.get(symbol).copied().unwrap_or(0.0);
                quantity * price
            })
            .sum();

        stocks_value + self.cash
    }
}

/// Historical price point
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PriceHistory {
    pub symbol: String,
    pub timestamp: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: u64,
}

/// Market statistics
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MarketStats {
    pub total_symbols: usize,
    pub total_volume: u64,
    pub up_count: usize,
    pub down_count: usize,
    pub unchanged_count: usize,
    pub last_update: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_portfolio_operations() {
        let mut portfolio = Portfolio::new("trader1".to_string(), 10000.0);

        // Buy stock
        portfolio.buy("AAPL".to_string(), 10.0, 150.0).unwrap();
        assert_eq!(portfolio.cash, 10000.0 - 1500.0);
        assert_eq!(*portfolio.holdings.get("AAPL").unwrap(), 10.0);

        // Sell stock
        portfolio.sell("AAPL".to_string(), 5.0, 150.0).unwrap();
        assert_eq!(portfolio.cash, 10000.0 - 750.0);
        assert_eq!(*portfolio.holdings.get("AAPL").unwrap(), 5.0);
    }

    #[test]
    fn test_portfolio_value() {
        let portfolio = Portfolio::new("trader1".to_string(), 5000.0);
        let mut prices = HashMap::new();
        prices.insert("AAPL".to_string(), 150.0);

        let value = portfolio.get_value(&prices);
        assert_eq!(value, 5000.0);
    }

    #[test]
    fn test_insufficient_funds() {
        let mut portfolio = Portfolio::new("trader1".to_string(), 1000.0);
        let result = portfolio.buy("AAPL".to_string(), 100.0, 150.0);

        assert!(result.is_err());
    }
}
