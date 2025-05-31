# MM Trader

## Overview

### Components

#### Data System

Backend system for different data sources used for the trading models.

For example:

1. Price Feeds
2. Blockchain Data
3. Economic Data

#### Strategies

* Strategies can be implemented by 1) trader, 2) backtester.
* It must have configuration to identify:
    * Assets that it will be trading.
    * General restrictions: (i.e. tripple barrier method, general limits).
    * Features Set
    * Prices
    * Rules that it must follow.

* There is a strategy registry:
    1. Will need to have an ability to point to a model, have it know which data sets are dependencies, ability to use the startegy (i.e. trader or backtester).
    2. Place to register new strategies.

* 

#### Backtester

* Requests data from historical feeds

#### Portfolio Manager

#### Trading Bot

* Can be either:
    1) Paper Trader
    2) Real-time Trader

#### Front End

* Login with user Authentication and Authorization
