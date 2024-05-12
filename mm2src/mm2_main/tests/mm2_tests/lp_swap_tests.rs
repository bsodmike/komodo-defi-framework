use common::{block_on, log};
use http::HeaderMap;
use http::StatusCode;
use mm2_test_helpers::for_tests::MarketMakerIt;
use serde_json::json;

pub mod seed_metrics {
    use super::*;
    use mm2_main::mm2::lp_swap::seed_metrics::{save_to_disk, SerializeFormat};

    // dummy test helping IDE to recognize this as test module
    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn dummy() { assert!(true) }

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn fake_data() {
        /// Fake market data
        ///
        /// * Timestamp in miliseconds
        /// * Open price for the day
        /// * Highest price for the day
        /// * Lowest price for the day
        /// * Close price for the dayx
        fn get_fake_data() -> Vec<(i64, f32, f32, f32, f32)> {
            let bytes = include_bytes!("fixtures/seed_metrics/market_fake_data.json");
            let data: Result<Vec<(i64, f32, f32, f32, f32)>, mm2_io::fs::postcard::postcard::Error> =
                mm2_io::fs::postcard::postcard::from_bytes(bytes);

            data.unwrap()
        }

        let fake_data = get_fake_data();
        let output_dir = "/home/mike/work/TillyHK/";

        let x = block_on(save_to_disk(
            &fake_data,
            "output",
            output_dir,
            &SerializeFormat::Json,
            None,
        ));
    }
}
