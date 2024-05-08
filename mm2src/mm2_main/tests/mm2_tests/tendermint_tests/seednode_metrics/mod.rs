use crate::integration_tests_common::{enable_electrum, enable_electrum_json};
use crate::mm2_tests::tendermint_tests::seednode_metrics::util::{node_test_config, seednode_test_config,
                                                                 SeednodeConfig};
use crate::mm2_tests::tendermint_tests::*;
use common::executor::Timer;
use common::log::{self, debug};
use instant::Duration;
use mm2_rpc::data::legacy::OrderbookResponse;
use mm2_test_helpers::for_tests::{check_my_swap_status, check_recent_swaps, doc_conf, iris_nimda_testnet_conf,
                                  iris_testnet_conf, nucleus_testnet_conf, usdc_ibc_iris_testnet_conf,
                                  wait_check_stats_swap_status, DOC_ELECTRUM_ADDRS};
use serde_json::{self as json, json, Value as Json};
use std::convert::TryFrom;
use std::{env, thread};

const BOB_PASSPHRASE: &str = "iris test seed";
const ALICE_PASSPHRASE: &str = "iris test2 seed";

mod util {
    use super::*;

    pub const DEFAULT_RPC_PASSWORD: &str = "password";

    /// Due to `enable_electrum`, far all intents as purposes, being marked as deprecated
    pub(crate) fn get_doc_electrum_addrs() -> Vec<Json> {
        DOC_ELECTRUM_ADDRS
            .iter()
            .map(|elm| json!({"url": elm.to_owned()}))
            .collect()
    }

    pub struct Mm2TestConf {
        pub conf: Json,
        pub rpc_password: String,
    }

    #[derive(Debug, Default)]
    pub struct SeednodeConfig {
        pub trading_proto_v2: bool,
        pub i_am_metric: bool,
    }

    impl SeednodeConfig {
        pub fn new() -> Self { Self { ..Default::default() } }
    }

    /// Generates a seed node conf
    ///
    /// * `use_trading_proto_v2` - Boolean
    pub fn seednode_test_config<C>(
        passphrase: &str,
        netid: &usize,
        coins: &Json,
        ip_env_var: &str,
        port_env_var: &str,
        config: C,
    ) -> Mm2TestConf
    where
        C: FnOnce(SeednodeConfig) -> SeednodeConfig,
    {
        let c = config(SeednodeConfig::new());
        log!("🧪 seednode_test_config:\n\n\n");
        dbg!(&c);
        log!("\n\n\n🧪");

        Mm2TestConf {
            conf: json!({
                "gui": "nogui",
                "netid": netid,
                "dht": "on",
                "myipaddr": env::var(ip_env_var).ok(),
                "rpcip": env::var(ip_env_var).ok(),
                "canbind": env::var(port_env_var).ok().map (|s| s.parse::<i64>().unwrap()),
                "passphrase": passphrase,
                "coins": coins,
                "rpc_password": "password",
                "use_trading_proto_v2": c.trading_proto_v2,
                "i_am_seed": true,
                "i_am_metric": c.i_am_metric,
            }),
            rpc_password: DEFAULT_RPC_PASSWORD.into(),
        }
    }

    /// Generates a node conf
    pub fn node_test_config(
        passphrase: &str,
        netid: &usize,
        coins: &Json,
        ip_env_var: &str,
        seed_node_addr: &str,
    ) -> Mm2TestConf {
        Mm2TestConf {
            conf: json!({
                "gui": "nogui",
                "netid": netid,
                "dht": "on",
                "myipaddr": env::var(ip_env_var).ok(),
                "rpcip": env::var(ip_env_var).ok(),
                "passphrase": passphrase,
                "coins": coins,
                "seednodes": [seed_node_addr.to_string()],
                "rpc_password": "password",
                "skip_startup_checks": true,
            }),
            rpc_password: DEFAULT_RPC_PASSWORD.into(),
        }
    }

    pub async fn my_swap_status(mm: &MarketMakerIt, uuid: &str) -> Result<Json, String> {
        let response = mm
            .rpc(&json!({
                "userpass": mm.userpass,
                "method": "my_swap_status",
                "params": {
                    "uuid": uuid,
                }
            }))
            .await
            .unwrap();

        if !response.0.is_success() {
            return Err(format!("!status of {}: {}", uuid, response.1));
        }

        Ok(json::from_str(&response.1).unwrap())
    }
}

mod without_metrics {
    use super::*;

    pub async fn verify_swap_metrics(mm_bob: &MarketMakerIt, mm_alice: &MarketMakerIt) -> Result<(), String> {
        // let rc = mm_alice
        //     .rpc(&json!({
        //         "userpass": mm_alice.userpass,
        //         "method": "metrics",

        //     }))
        //     .await
        //     .unwrap();
        // assert!(rc.0.is_success(), "!buy: {}", rc.1);
        // let metrics_json: serde_json::Value = serde_json::from_str(&rc.1).unwrap();

        // log!("🧪\n\n\n");
        // dbg!(metrics_json);
        // log!("\n\n\n🧪");

        Ok(())
    }

    pub async fn trade_base_rel_tendermint(
        mut mm_bob: MarketMakerIt,
        mut mm_alice: MarketMakerIt,
        base: &str,
        rel: &str,
        maker_price: i32,
        taker_price: i32,
        volume: f64,
    ) {
        log!("Issue bob {}/{} sell request", base, rel);
        let rc = mm_bob
            .rpc(&json!({
                "userpass": mm_bob.userpass,
                "method": "setprice",
                "base": base,
                "rel": rel,
                "price": maker_price,
                "volume": volume
            }))
            .await
            .unwrap();
        assert!(rc.0.is_success(), "!setprice: {}", rc.1);

        let mut uuids = vec![];

        common::log::info!(
            "Trigger alice subscription to {}/{} orderbook topic first and sleep for 1 second",
            base,
            rel
        );
        let rc = mm_alice
            .rpc(&json!({
                "userpass": mm_alice.userpass,
                "method": "orderbook",
                "base": base,
                "rel": rel,
            }))
            .await
            .unwrap();
        assert!(rc.0.is_success(), "!orderbook: {}", rc.1);
        Timer::sleep(1.).await;
        common::log::info!("Issue alice {}/{} buy request", base, rel);
        let rc = mm_alice
            .rpc(&json!({
                "userpass": mm_alice.userpass,
                "method": "buy",
                "base": base,
                "rel": rel,
                "volume": volume,
                "price": taker_price
            }))
            .await
            .unwrap();
        assert!(rc.0.is_success(), "!buy: {}", rc.1);
        let buy_json: serde_json::Value = serde_json::from_str(&rc.1).unwrap();
        uuids.push(buy_json["result"]["uuid"].as_str().unwrap().to_owned());

        // ensure the swaps are started
        let expected_log = format!("Entering the taker_swap_loop {}/{}", base, rel);
        mm_alice
            .wait_for_log(5., |log| log.contains(&expected_log))
            .await
            .unwrap();
        let expected_log = format!("Entering the maker_swap_loop {}/{}", base, rel);
        mm_bob
            .wait_for_log(5., |log| log.contains(&expected_log))
            .await
            .unwrap();

        for uuid in uuids.iter() {
            // ensure the swaps are indexed to the SQLite database
            let expected_log = format!("Inserting new swap {} to the SQLite database", uuid);
            mm_alice
                .wait_for_log(5., |log| log.contains(&expected_log))
                .await
                .unwrap();
            mm_bob
                .wait_for_log(5., |log| log.contains(&expected_log))
                .await
                .unwrap()
        }

        for uuid in uuids.iter() {
            match mm_bob
                .wait_for_log(180., |log| log.contains(&format!("[swap uuid={}] Finished", uuid)))
                .await
            {
                Ok(_) => (),
                Err(_) => {
                    log!("{}", mm_bob.log_as_utf8().unwrap());
                },
            }

            match mm_alice
                .wait_for_log(180., |log| log.contains(&format!("[swap uuid={}] Finished", uuid)))
                .await
            {
                Ok(_) => (),
                Err(_) => {
                    log!("{}", mm_alice.log_as_utf8().unwrap());
                },
            }

            log!("Waiting a few second for the fresh swap status to be saved..");
            Timer::sleep(5.).await;

            log!("{}", mm_alice.log_as_utf8().unwrap());
            log!("Checking alice/taker status..");
            check_my_swap_status(
                &mm_alice,
                uuid,
                BigDecimal::try_from(volume).unwrap(),
                BigDecimal::try_from(volume).unwrap(),
            )
            .await;

            log!("{}", mm_bob.log_as_utf8().unwrap());
            log!("Checking bob/maker status..");
            check_my_swap_status(
                &mm_bob,
                uuid,
                BigDecimal::try_from(volume).unwrap(),
                BigDecimal::try_from(volume).unwrap(),
            )
            .await;
        }

        verify_swap_metrics(&mm_bob, &mm_alice).await.unwrap();

        log!("Waiting 3 seconds for nodes to broadcast their swaps data..");
        Timer::sleep(3.).await;

        for uuid in uuids.iter() {
            log!("Checking alice status..");
            wait_check_stats_swap_status(&mm_alice, uuid, 30).await;

            log!("Checking bob status..");
            wait_check_stats_swap_status(&mm_bob, uuid, 30).await;
        }

        log!("Checking alice recent swaps..");
        check_recent_swaps(&mm_alice, uuids.len()).await;
        log!("Checking bob recent swaps..");
        check_recent_swaps(&mm_bob, uuids.len()).await;
        log!("Get {}/{} orderbook", base, rel);
        let rc = mm_bob
            .rpc(&json!({
                "userpass": mm_bob.userpass,
                "method": "orderbook",
                "base": base,
                "rel": rel,
            }))
            .await
            .unwrap();
        assert!(rc.0.is_success(), "!orderbook: {}", rc.1);

        let bob_orderbook: OrderbookResponse = serde_json::from_str(&rc.1).unwrap();
        log!("{}/{} orderbook {:?}", base, rel, bob_orderbook);

        assert_eq!(0, bob_orderbook.bids.len(), "{} {} bids must be empty", base, rel);
        assert_eq!(0, bob_orderbook.asks.len(), "{} {} asks must be empty", base, rel);

        mm_bob.stop().await.unwrap();
        mm_alice.stop().await.unwrap();
    }

    #[test]
    fn swap_usdc_ibc_with_nimda() {
        let bob_passphrase = String::from(BOB_PASSPHRASE);
        let alice_passphrase = String::from(ALICE_PASSPHRASE);

        let coins = json!([
            usdc_ibc_iris_testnet_conf(),
            iris_nimda_testnet_conf(),
            iris_testnet_conf(),
        ]);

        // seed node
        let settings = |mut c: util::SeednodeConfig| -> SeednodeConfig {
            c.trading_proto_v2 = false;
            c.i_am_metric = false;

            c
        };
        let conf = seednode_test_config(
            bob_passphrase.as_str(),
            &8999,
            &coins,
            "BOB_TRADE_IP",
            "BOB_TRADE_PORT",
            settings,
        );
        let mut mm_bob = MarketMakerIt::start(conf.conf, "password".into(), None).unwrap();

        thread::sleep(Duration::from_secs(1));

        // normal node
        let conf = node_test_config(
            // ra format as block
            alice_passphrase.as_str(),
            &8999,
            &coins,
            "ALICE_TRADE_IP",
            &mm_bob.my_seed_addr().as_str(),
        );
        let mm_alice = MarketMakerIt::start(conf.conf, "password".into(), None).unwrap();

        thread::sleep(Duration::from_secs(1));

        // enable tendermint, ensuring we have funds to spend
        let zero_balance: BigDecimal = "0.".parse().unwrap();
        let activation_result = block_on(enable_tendermint(
            &mm_bob,
            "IRIS-TEST",
            &["IRIS-NIMDA", "USDC-IBC-IRIS"],
            IRIS_TESTNET_RPC_URLS,
            false,
        ));
        let result: RpcV2Response<TendermintActivationResult> = serde_json::from_value(activation_result).unwrap();
        assert_ne!(result.result.balance.unwrap().spendable, zero_balance);

        let activation_result = block_on(enable_tendermint(
            &mm_alice,
            "IRIS-TEST",
            &["IRIS-NIMDA", "USDC-IBC-IRIS"],
            IRIS_TESTNET_RPC_URLS,
            false,
        ));
        let result: RpcV2Response<TendermintActivationResult> = serde_json::from_value(activation_result).unwrap();
        assert_ne!(result.result.balance.unwrap().spendable, zero_balance);

        // Ensure seednode startup with metrics is not detected
        let expected_log = format!("lp_init: Seednode startup with metrics detected");
        let mm_bob_mut = &mut mm_bob;
        let _ = block_on(mm_bob_mut.wait_for_log(5., |log| !log.contains(&expected_log))).expect("!wait_for_log");

        block_on(trade_base_rel_tendermint(
            mm_bob,
            mm_alice,
            "USDC-IBC-IRIS",
            "IRIS-NIMDA",
            1,
            2,
            0.008,
        ));
    }
}

mod trading_proto_v1 {
    use super::*;

    pub async fn verify_swap_metrics(mm_bob: &MarketMakerIt, mm_alice: &MarketMakerIt) -> Result<(), String> {
        // let rc = mm_alice
        //     .rpc(&json!({
        //         "userpass": mm_alice.userpass,
        //         "method": "metrics",

        //     }))
        //     .await
        //     .unwrap();
        // assert!(rc.0.is_success(), "!buy: {}", rc.1);
        // let metrics_json: serde_json::Value = serde_json::from_str(&rc.1).unwrap();

        // log!("🧪\n\n\n");
        // dbg!(metrics_json);
        // log!("\n\n\n🧪");

        Ok(())
    }

    pub async fn trade_base_rel_tendermint(
        mut mm_bob: MarketMakerIt,
        mut mm_alice: MarketMakerIt,
        base: &str,
        rel: &str,
        maker_price: i32,
        taker_price: i32,
        volume: f64,
    ) {
        log!("Issue bob {}/{} sell request", base, rel);
        let rc = mm_bob
            .rpc(&json!({
                "userpass": mm_bob.userpass,
                "method": "setprice",
                "base": base,
                "rel": rel,
                "price": maker_price,
                "volume": volume
            }))
            .await
            .unwrap();
        assert!(rc.0.is_success(), "!setprice: {}", rc.1);

        let mut uuids = vec![];

        common::log::info!(
            "Trigger alice subscription to {}/{} orderbook topic first and sleep for 1 second",
            base,
            rel
        );
        let rc = mm_alice
            .rpc(&json!({
                "userpass": mm_alice.userpass,
                "method": "orderbook",
                "base": base,
                "rel": rel,
            }))
            .await
            .unwrap();
        assert!(rc.0.is_success(), "!orderbook: {}", rc.1);
        Timer::sleep(1.).await;
        common::log::info!("Issue alice {}/{} buy request", base, rel);
        let rc = mm_alice
            .rpc(&json!({
                "userpass": mm_alice.userpass,
                "method": "buy",
                "base": base,
                "rel": rel,
                "volume": volume,
                "price": taker_price
            }))
            .await
            .unwrap();
        assert!(rc.0.is_success(), "!buy: {}", rc.1);
        let buy_json: serde_json::Value = serde_json::from_str(&rc.1).unwrap();
        uuids.push(buy_json["result"]["uuid"].as_str().unwrap().to_owned());

        // ensure the swaps are started
        let expected_log = format!("Entering the taker_swap_loop {}/{}", base, rel);
        mm_alice
            .wait_for_log(5., |log| log.contains(&expected_log))
            .await
            .unwrap();
        let expected_log = format!("Entering the maker_swap_loop {}/{}", base, rel);
        mm_bob
            .wait_for_log(5., |log| log.contains(&expected_log))
            .await
            .unwrap();

        for uuid in uuids.iter() {
            // ensure the swaps are indexed to the SQLite database
            let expected_log = format!("Inserting new swap {} to the SQLite database", uuid);
            mm_alice
                .wait_for_log(5., |log| log.contains(&expected_log))
                .await
                .unwrap();
            mm_bob
                .wait_for_log(5., |log| log.contains(&expected_log))
                .await
                .unwrap()
        }

        for uuid in uuids.iter() {
            match mm_bob
                .wait_for_log(180., |log| log.contains(&format!("[swap uuid={}] Finished", uuid)))
                .await
            {
                Ok(_) => (),
                Err(_) => {
                    log!("{}", mm_bob.log_as_utf8().unwrap());
                },
            }

            match mm_alice
                .wait_for_log(180., |log| log.contains(&format!("[swap uuid={}] Finished", uuid)))
                .await
            {
                Ok(_) => (),
                Err(_) => {
                    log!("{}", mm_alice.log_as_utf8().unwrap());
                },
            }

            log!("Waiting a few second for the fresh swap status to be saved..");
            Timer::sleep(5.).await;

            log!("{}", mm_alice.log_as_utf8().unwrap());
            log!("Checking alice/taker status..");
            check_my_swap_status(
                &mm_alice,
                uuid,
                BigDecimal::try_from(volume).unwrap(),
                BigDecimal::try_from(volume).unwrap(),
            )
            .await;

            log!("{}", mm_bob.log_as_utf8().unwrap());
            log!("Checking bob/maker status..");
            check_my_swap_status(
                &mm_bob,
                uuid,
                BigDecimal::try_from(volume).unwrap(),
                BigDecimal::try_from(volume).unwrap(),
            )
            .await;
        }

        verify_swap_metrics(&mm_bob, &mm_alice).await.unwrap();

        log!("Waiting 3 seconds for nodes to broadcast their swaps data..");
        Timer::sleep(3.).await;

        for uuid in uuids.iter() {
            log!("Checking alice status..");
            wait_check_stats_swap_status(&mm_alice, uuid, 30).await;

            log!("Checking bob status..");
            wait_check_stats_swap_status(&mm_bob, uuid, 30).await;
        }

        log!("Checking alice recent swaps..");
        check_recent_swaps(&mm_alice, uuids.len()).await;
        log!("Checking bob recent swaps..");
        check_recent_swaps(&mm_bob, uuids.len()).await;
        log!("Get {}/{} orderbook", base, rel);
        let rc = mm_bob
            .rpc(&json!({
                "userpass": mm_bob.userpass,
                "method": "orderbook",
                "base": base,
                "rel": rel,
            }))
            .await
            .unwrap();
        assert!(rc.0.is_success(), "!orderbook: {}", rc.1);

        let bob_orderbook: OrderbookResponse = serde_json::from_str(&rc.1).unwrap();
        log!("{}/{} orderbook {:?}", base, rel, bob_orderbook);

        assert_eq!(0, bob_orderbook.bids.len(), "{} {} bids must be empty", base, rel);
        assert_eq!(0, bob_orderbook.asks.len(), "{} {} asks must be empty", base, rel);

        mm_bob.stop().await.unwrap();
        mm_alice.stop().await.unwrap();
    }

    #[test]
    fn swap_usdc_ibc_with_nimda() {
        let bob_passphrase = String::from(BOB_PASSPHRASE);
        let alice_passphrase = String::from(ALICE_PASSPHRASE);

        let coins = json!([
            usdc_ibc_iris_testnet_conf(),
            iris_nimda_testnet_conf(),
            iris_testnet_conf(),
        ]);

        // seed node
        let settings = |mut c: util::SeednodeConfig| -> SeednodeConfig {
            c.trading_proto_v2 = false;
            c.i_am_metric = true;

            c
        };
        let conf = seednode_test_config(
            bob_passphrase.as_str(),
            &8999,
            &coins,
            "BOB_TRADE_IP",
            "BOB_TRADE_PORT",
            settings,
        );
        let mut mm_bob = MarketMakerIt::start(conf.conf, "password".into(), None).unwrap();

        thread::sleep(Duration::from_secs(1));

        // normal node
        let conf = node_test_config(
            // ra format as block
            alice_passphrase.as_str(),
            &8999,
            &coins,
            "ALICE_TRADE_IP",
            &mm_bob.my_seed_addr().as_str(),
        );
        let mm_alice = MarketMakerIt::start(conf.conf, "password".into(), None).unwrap();

        thread::sleep(Duration::from_secs(1));

        // enable tendermint, ensuring we have funds to spend
        let zero_balance: BigDecimal = "0.".parse().unwrap();
        let activation_result = block_on(enable_tendermint(
            &mm_bob,
            "IRIS-TEST",
            &["IRIS-NIMDA", "USDC-IBC-IRIS"],
            IRIS_TESTNET_RPC_URLS,
            false,
        ));
        let result: RpcV2Response<TendermintActivationResult> = serde_json::from_value(activation_result).unwrap();
        assert_ne!(result.result.balance.unwrap().spendable, zero_balance);

        let activation_result = block_on(enable_tendermint(
            &mm_alice,
            "IRIS-TEST",
            &["IRIS-NIMDA", "USDC-IBC-IRIS"],
            IRIS_TESTNET_RPC_URLS,
            false,
        ));
        let result: RpcV2Response<TendermintActivationResult> = serde_json::from_value(activation_result).unwrap();
        assert_ne!(result.result.balance.unwrap().spendable, zero_balance);

        // Ensure seednode startup with metrics is detected
        let expected_log = format!("lp_init: Seednode startup with metrics detected");
        let mm_bob_mut = &mut mm_bob;
        let _ = block_on(mm_bob_mut.wait_for_log(5., |log| log.contains(&expected_log))).expect("!wait_for_log");

        block_on(trade_base_rel_tendermint(
            mm_bob,
            mm_alice,
            "USDC-IBC-IRIS",
            "IRIS-NIMDA",
            1,
            2,
            0.008,
        ));
    }
}

mod trading_proto_v2 {
    use super::*;

    pub async fn verify_swap_metrics(mm_bob: &MarketMakerIt, mm_alice: &MarketMakerIt) -> Result<(), String> {
        // let rc = mm_alice
        //     .rpc(&json!({
        //         "userpass": mm_alice.userpass,
        //         "method": "metrics",
        //     }))
        //     .await
        //     .unwrap();
        // assert!(rc.0.is_success(), "!buy: {}", rc.1);
        // let metrics_json: serde_json::Value = serde_json::from_str(&rc.1).unwrap();

        // log!("🧪\n\n\n");
        // dbg!(metrics_json);
        // log!("\n\n\n🧪");

        Ok(())
    }

    pub async fn trade_base_rel_tendermint(
        mut mm_bob: MarketMakerIt,
        mut mm_alice: MarketMakerIt,
        base: &str,
        rel: &str,
        maker_price: i32,
        taker_price: i32,
        volume: f64,
    ) {
        log!("Issue bob {}/{} sell request", base, rel);
        let rc = mm_bob
            .rpc(&json!({
                "userpass": mm_bob.userpass,
                "method": "setprice",
                "base": base,
                "rel": rel,
                "price": maker_price,
                "volume": volume
            }))
            .await
            .unwrap();
        assert!(rc.0.is_success(), "!setprice: {}", rc.1);

        let mut uuids = vec![];

        common::log::info!(
            "Trigger alice subscription to {}/{} orderbook topic first and sleep for 1 second",
            base,
            rel
        );
        let rc = mm_alice
            .rpc(&json!({
                "userpass": mm_alice.userpass,
                "method": "orderbook",
                "base": base,
                "rel": rel,
            }))
            .await
            .unwrap();
        assert!(rc.0.is_success(), "!orderbook: {}", rc.1);
        Timer::sleep(1.).await;
        common::log::info!("Issue alice {}/{} buy request", base, rel);
        let rc = mm_alice
            .rpc(&json!({
                "userpass": mm_alice.userpass,
                "method": "buy",
                "base": base,
                "rel": rel,
                "volume": volume,
                "price": taker_price
            }))
            .await
            .unwrap();
        assert!(rc.0.is_success(), "!buy: {}", rc.1);
        let buy_json: serde_json::Value = serde_json::from_str(&rc.1).unwrap();
        uuids.push(buy_json["result"]["uuid"].as_str().unwrap().to_owned());

        // ensure the swaps are started
        let expected_log = format!("Entering the taker_swap_loop {}/{}", base, rel);
        mm_alice
            .wait_for_log(5., |log| log.contains(&expected_log))
            .await
            .unwrap();
        let expected_log = format!("Entering the maker_swap_loop {}/{}", base, rel);
        mm_bob
            .wait_for_log(5., |log| log.contains(&expected_log))
            .await
            .unwrap();

        // Ensure trading protocol v2 is enabled.
        let expected_log = format!("alice: use_trading_proto_v2: enabled");
        mm_alice
            .wait_for_log(5., |log| log.contains(&expected_log))
            .await
            .unwrap();
        let expected_log = format!("bob: use_trading_proto_v2: enabled");
        mm_bob
            .wait_for_log(5., |log| log.contains(&expected_log))
            .await
            .unwrap();

        for uuid in uuids.iter() {
            // ensure the swaps are indexed to the SQLite database
            let expected_log = format!("Inserting new swap v2 {} to the SQLite database", uuid);
            mm_alice
                .wait_for_log(5., |log| log.contains(&expected_log))
                .await
                .unwrap();
            mm_bob
                .wait_for_log(5., |log| log.contains(&expected_log))
                .await
                .unwrap()
        }

        for uuid in uuids.iter() {
            match mm_bob
                .wait_for_log(180., |log| log.contains(&format!("[swap uuid={}] Finished", uuid)))
                .await
            {
                Ok(_) => (),
                Err(_) => {
                    log!("{}", mm_bob.log_as_utf8().unwrap());
                },
            }

            match mm_alice
                .wait_for_log(180., |log| log.contains(&format!("[swap uuid={}] Finished", uuid)))
                .await
            {
                Ok(_) => (),
                Err(_) => {
                    log!("{}", mm_alice.log_as_utf8().unwrap());
                },
            }

            log!("Waiting a few second for the fresh swap status to be saved..");
            Timer::sleep(5.).await;

            log!("{}", mm_alice.log_as_utf8().unwrap());
            log!("Checking alice/taker status..");
            check_my_swap_status(
                &mm_alice,
                uuid,
                BigDecimal::try_from(volume).unwrap(),
                BigDecimal::try_from(volume).unwrap(),
            )
            .await;

            log!("{}", mm_bob.log_as_utf8().unwrap());
            log!("Checking bob/maker status..");
            check_my_swap_status(
                &mm_bob,
                uuid,
                BigDecimal::try_from(volume).unwrap(),
                BigDecimal::try_from(volume).unwrap(),
            )
            .await;
        }

        verify_swap_metrics(&mm_bob, &mm_alice).await.unwrap();

        log!("Waiting 3 seconds for nodes to broadcast their swaps data..");
        Timer::sleep(3.).await;

        for uuid in uuids.iter() {
            log!("Checking alice status..");
            wait_check_stats_swap_status(&mm_alice, uuid, 30).await;

            log!("Checking bob status..");
            wait_check_stats_swap_status(&mm_bob, uuid, 30).await;
        }

        log!("Checking alice recent swaps..");
        check_recent_swaps(&mm_alice, uuids.len()).await;
        log!("Checking bob recent swaps..");
        check_recent_swaps(&mm_bob, uuids.len()).await;
        log!("Get {}/{} orderbook", base, rel);
        let rc = mm_bob
            .rpc(&json!({
                "userpass": mm_bob.userpass,
                "method": "orderbook",
                "base": base,
                "rel": rel,
            }))
            .await
            .unwrap();
        assert!(rc.0.is_success(), "!orderbook: {}", rc.1);

        let bob_orderbook: OrderbookResponse = serde_json::from_str(&rc.1).unwrap();
        log!("{}/{} orderbook {:?}", base, rel, bob_orderbook);

        assert_eq!(0, bob_orderbook.bids.len(), "{} {} bids must be empty", base, rel);
        assert_eq!(0, bob_orderbook.asks.len(), "{} {} asks must be empty", base, rel);

        mm_bob.stop().await.unwrap();
        mm_alice.stop().await.unwrap();
    }

    #[test]
    #[ignore] // FIXME: temporarily on hold, to focus on `process_swap_msg` (v1).
    fn swap_usdc_ibc_with_nimda() {
        let bob_passphrase = String::from(BOB_PASSPHRASE);
        let alice_passphrase = String::from(ALICE_PASSPHRASE);

        let coins = json!([
            usdc_ibc_iris_testnet_conf(),
            iris_nimda_testnet_conf(),
            iris_testnet_conf(),
        ]);

        // seed node
        let settings = |mut c: util::SeednodeConfig| -> SeednodeConfig {
            c.trading_proto_v2 = true;
            c.i_am_metric = true;

            c
        };
        let conf = seednode_test_config(
            bob_passphrase.as_str(),
            &8999,
            &coins,
            "BOB_TRADE_IP",
            "BOB_TRADE_PORT",
            settings,
        );
        let mut mm_bob = MarketMakerIt::start(conf.conf, "password".into(), None).unwrap();

        thread::sleep(Duration::from_secs(1));

        // normal node
        let conf = node_test_config(
            // ra format as block
            alice_passphrase.as_str(),
            &8999,
            &coins,
            "ALICE_TRADE_IP",
            &mm_bob.my_seed_addr().as_str(),
        );
        let mm_alice = MarketMakerIt::start(conf.conf, "password".into(), None).unwrap();

        thread::sleep(Duration::from_secs(1));

        // enable tendermint, ensuring we have funds to spend
        let zero_balance: BigDecimal = "0.".parse().unwrap();
        let activation_result = block_on(enable_tendermint(
            &mm_bob,
            "IRIS-TEST",
            &["IRIS-NIMDA", "USDC-IBC-IRIS"],
            IRIS_TESTNET_RPC_URLS,
            false,
        ));
        let result: RpcV2Response<TendermintActivationResult> = serde_json::from_value(activation_result).unwrap();
        assert_ne!(result.result.balance.unwrap().spendable, zero_balance);

        let activation_result = block_on(enable_tendermint(
            &mm_alice,
            "IRIS-TEST",
            &["IRIS-NIMDA", "USDC-IBC-IRIS"],
            IRIS_TESTNET_RPC_URLS,
            false,
        ));
        let result: RpcV2Response<TendermintActivationResult> = serde_json::from_value(activation_result).unwrap();
        assert_ne!(result.result.balance.unwrap().spendable, zero_balance);

        // Ensure seednode startup with metrics is detected
        let expected_log = format!("lp_init: Seednode startup with metrics detected");
        let mm_bob_mut = &mut mm_bob;
        let _ = block_on(mm_bob_mut.wait_for_log(5., |log| log.contains(&expected_log))).expect("!wait_for_log");

        block_on(trade_base_rel_tendermint(
            mm_bob,
            mm_alice,
            "USDC-IBC-IRIS",
            "IRIS-NIMDA",
            1,
            2,
            0.008,
        ));
    }
}
