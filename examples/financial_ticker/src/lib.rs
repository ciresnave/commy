// Commy Financial Ticker System - Shared library
//
// This module defines the types and calculations for market data distribution

use serde::{Deserialize, Serialize};

/// Stock price information
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StockPrice {
    pub symbol: String,
    pub price: f64,
    pub bid: f64,
    pub ask: f64,
    pub volume: u64,
    pub volume_24h: u64,
    pub timestamp: i64,
    pub previous_close: f64,
}

impl StockPrice {
    pub fn change(&self) -> f64 {
        self.price - self.previous_close
    }

    pub fn change_percent(&self) -> f64 {
        if self.previous_close == 0.0 {
            0.0
        } else {
            (self.change() / self.previous_close) * 100.0
        }
    }

    pub fn spread(&self) -> f64 {
        self.ask - self.bid
    }

    pub fn spread_percent(&self) -> f64 {
        if self.bid == 0.0 {
            0.0
        } else {
            (self.spread() / self.bid) * 100.0
        }
    }
}

/// Market index (like S&P 500)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MarketIndex {
    pub name: String,
    pub symbol: String,
    pub value: f64,
    pub components: u32,
    pub timestamp: i64,
    pub previous_close: f64,
}

impl MarketIndex {
    pub fn change(&self) -> f64 {
        self.value - self.previous_close
    }

    pub fn change_percent(&self) -> f64 {
        if self.previous_close == 0.0 {
            0.0
        } else {
            (self.change() / self.previous_close) * 100.0
        }
    }
}

/// Alert condition and status
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Alert {
    pub id: String,
    pub symbol: String,
    pub alert_type: AlertType,
    pub condition: String,
    pub triggered_at: i64,
    pub severity: AlertSeverity,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum AlertType {
    PriceThreshold,
    VolumeSpike,
    CircuitBreaker,
    IndexMovement,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum AlertSeverity {
    Info,
    Warning,
    Critical,
}

/// Market state
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum MarketState {
    PreMarket,
    Open,
    Halted,
    Closed,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stock_price_calculations() {
        let stock = StockPrice {
            symbol: "AAPL".to_string(),
            price: 150.50,
            bid: 150.49,
            ask: 150.51,
            volume: 1000000,
            volume_24h: 50000000,
            timestamp: 1000,
            previous_close: 150.00,
        };

        assert!((stock.change() - 0.50).abs() < 0.01);
        assert!((stock.change_percent() - 0.333).abs() < 0.01);
        assert!((stock.spread() - 0.02).abs() < 0.001);
    }

    #[test]
    fn test_index_calculations() {
        let index = MarketIndex {
            name: "S&P 500".to_string(),
            symbol: "SPX".to_string(),
            value: 4900.50,
            components: 500,
            timestamp: 1000,
            previous_close: 4890.00,
        };

        assert!((index.change() - 10.50).abs() < 0.01);
        assert!((index.change_percent() - 0.215).abs() < 0.01);
    }
}
