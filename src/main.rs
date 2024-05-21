use anyhow::Result;
use chrono::{Datelike, Duration, NaiveDate};
use futures::StreamExt;
use regex::Regex;
use reqwest::Url;
use std::collections::HashSet;
use std::fs::File;
use std::io::Write;
use voyager::scraper::Selector;
use voyager::{Collector, Crawler, CrawlerConfig, Response, Scraper};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    struct Explorer {
        /// visited urls mapped with all the urls that link to that url
        visited: HashSet<Url>,
        link_selector: Selector,
        regex: Regex,
    }
    impl Default for Explorer {
        fn default() -> Self {
            Self {
                visited: Default::default(),
                link_selector: Selector::parse("a").unwrap(),
                regex: Regex::new(r"rmrb/html/202(\d)-(\d\d)/\d\d/.*.htm$").unwrap(),
            }
        }
    }

    impl Scraper for Explorer {
        type Output = (usize, Url);
        type State = Url;

        fn scrape(
            &mut self,
            response: Response<Self::State>,
            crawler: &mut Crawler<Self>,
        ) -> Result<Option<Self::Output>> {
            if !self.visited.insert(response.response_url.clone()) {
                return Ok(None);
            }
            response
                .html()
                .select(&self.link_selector)
                .filter_map(|link| link.value().attr("href"))
                .filter_map(|url| response.response_url.join(url).ok())
                .for_each(|mut url| {
                    url.set_fragment(None);
                    if !self.regex.is_match(url.as_str()) {
                        self.visited.insert(url);
                        return;
                    }
                    if self.visited.contains(&url) {
                        return;
                    }
                    crawler.visit(url);
                });

            Ok(Some((response.depth, response.response_url)))
        }
    }

    let config = CrawlerConfig::default().max_concurrent_requests(1_000);
    let mut collector = Collector::new(Explorer::default(), config);

    let start_date = NaiveDate::from_ymd_opt(2023, 4, 1).unwrap();
    let end_date = NaiveDate::from_ymd_opt(2024, 5, 21).unwrap();
    let mut current_date = start_date;
    while current_date <= end_date {
        let url = format!(
            "http://paper.people.com.cn/rmrb/html/{year}-{month:02}/{date:02}/nbs.D110000renmrb_01.htm",
            year = current_date.year(),
            month = current_date.month(),
            date = current_date.day()
        );
        println!(">>> {}", url);
        collector.crawler_mut().visit(url);
        current_date += Duration::days(1);
    }

    let article_regex = Regex::new(r"^http://paper\.people\.com\.cn/rmrb/html/202\d-\d\d/\d\d/nw.*htm$").unwrap();
    let mut f = File::options().append(true).create(true).open("urls.txt")?;
    while let Some(output) = collector.next().await {
        if let Ok((_, url)) = output {
            if article_regex.is_match(url.as_str()) {
                println!("Scraping: {}", url);
                writeln!(f, "{}", url)?;
            }
        }
    }

    Ok(())
}
