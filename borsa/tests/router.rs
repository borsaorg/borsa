mod helpers;

#[path = "router/analysis/router_analysis_not_found.rs"]
mod router_analysis_not_found;
#[path = "router/analysis/router_analysis_price_target.rs"]
mod router_analysis_price_target;
#[path = "router/analysis/router_analysis_recommendations.rs"]
mod router_analysis_recommendations;
#[path = "router/analysis/router_analysis_recommendations_summary.rs"]
mod router_analysis_recommendations_summary;
#[path = "router/analysis/router_analysis_upgrades_downgrades.rs"]
mod router_analysis_upgrades_downgrades;

#[path = "router/calendar/router_calendar.rs"]
mod router_calendar;

#[path = "router/core/router_fetch_strategies.rs"]
mod router_fetch_strategies;
#[path = "router/core/router_priority.rs"]
mod router_priority;

#[path = "router/download/router_download.rs"]
mod router_download;

#[path = "router/esg/router_esg.rs"]
mod router_esg;

#[path = "router/fundamentals/router_balance_sheet.rs"]
mod router_balance_sheet;
#[path = "router/fundamentals/router_cashflow.rs"]
mod router_cashflow;
#[path = "router/fundamentals/router_income_statement.rs"]
mod router_income_statement;

#[path = "router/history/router_history_attribution.rs"]
mod router_history_attribution;
#[path = "router/history/router_history_auto_resample.rs"]
mod router_history_auto_resample;
#[path = "router/history/router_history_empty_is_skipped.rs"]
mod router_history_empty_is_skipped;
#[path = "router/history/router_history_fallback.rs"]
mod router_history_fallback;
#[path = "router/history/router_history_interval_largest_divisor.rs"]
mod router_history_interval_largest_divisor;
#[path = "router/history/router_history_interval_passthrough.rs"]
mod router_history_interval_passthrough;
#[path = "router/history/router_history_interval_rounddown_resample.rs"]
mod router_history_interval_rounddown_resample;
#[path = "router/history/router_history_merge.rs"]
mod router_history_merge;
#[path = "router/history/router_history_merge_strategy.rs"]
mod router_history_merge_strategy;
#[path = "router/history/router_history_not_found.rs"]
mod router_history_not_found;
#[path = "router/history/router_history_prefer_adjusted.rs"]
mod router_history_prefer_adjusted;
#[path = "router/history/router_history_raw_close.rs"]
mod router_history_raw_close;
#[path = "router/history/router_history_resample_daily.rs"]
mod router_history_resample_daily;
#[path = "router/history/router_history_resample_weekly.rs"]
mod router_history_resample_weekly;
#[path = "router/history/router_history_validate.rs"]
mod router_history_validate;

#[path = "router/holders/router_holders.rs"]
mod router_holders;

#[path = "router/news/router_news.rs"]
mod router_news;

#[path = "router/options/router_option_chain.rs"]
mod router_option_chain;
#[path = "router/options/router_options_expirations.rs"]
mod router_options_expirations;

#[path = "router/profile/router_info.rs"]
mod router_info;

#[path = "router/quotes/router_kind_filter_quote.rs"]
mod router_kind_filter_quote;
#[path = "router/quotes/router_quote.rs"]
mod router_quote;
#[path = "router/quotes/router_quote_concurrency.rs"]
mod router_quote_concurrency;
#[path = "router/quotes/router_quote_not_found.rs"]
mod router_quote_not_found;
#[path = "router/quotes/router_quote_per_kind_priority.rs"]
mod router_quote_per_kind_priority;
#[path = "router/quotes/router_quote_provider_hot_swap.rs"]
mod router_quote_provider_hot_swap;
#[path = "router/quotes/router_quotes_fallback.rs"]
mod router_quotes_fallback;
#[path = "router/quotes/router_quotes_multi.rs"]
mod router_quotes_multi;

#[path = "router/search/router_search_kind_filter.rs"]
mod router_search_kind_filter;
#[path = "router/search/router_search_limit.rs"]
mod router_search_limit;
#[path = "router/search/router_search_priority.rs"]
mod router_search_priority;
#[path = "router/search/router_search_unsupported.rs"]
mod router_search_unsupported;

#[path = "router/stream/router_stream_backoff.rs"]
mod router_stream_backoff;
#[path = "router/stream/router_stream_downstream_drop.rs"]
mod router_stream_downstream_drop;
#[path = "router/stream/router_stream_failover_end.rs"]
mod router_stream_failover_end;
#[path = "router/stream/router_stream_kind_hint.rs"]
mod router_stream_kind_hint;
#[path = "router/stream/router_stream_no_provider.rs"]
mod router_stream_no_provider;
#[path = "router/stream/router_stream_quotes.rs"]
mod router_stream_quotes;
#[path = "router/stream/router_stream_quotes_multi.rs"]
mod router_stream_quotes_multi;
#[path = "router/stream/router_stream_startup_all_fail.rs"]
mod router_stream_startup_all_fail;
#[path = "router/stream/router_stream_startup_fallback.rs"]
mod router_stream_startup_fallback;
#[path = "router/stream/router_stream_symbol_filtering.rs"]
mod router_stream_symbol_filtering;
