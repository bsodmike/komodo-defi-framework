use crate::integration_tests_common::{enable_electrum, enable_electrum_json};
use crate::mm2_tests::tendermint_tests::seednode_metrics::util::seednode_trade_legacy_with_metrics;
use crate::mm2_tests::tendermint_tests::*;
use common::executor::Timer;
use common::log;
use instant::Duration;
use mm2_rpc::data::legacy::OrderbookResponse;
use mm2_test_helpers::for_tests::{check_my_swap_status, check_recent_swaps, doc_conf, nucleus_testnet_conf,
                                  wait_check_stats_swap_status, DOC_ELECTRUM_ADDRS};
use serde_json::{self as json, json, Value as Json};
use std::convert::TryFrom;
use std::{env, thread};

const BOB_PASSPHRASE: &str = "test seednode metrics";
const ALICE_PASSPHRASE: &str = "test2 seednode metrics";

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

    /// Generates a seed node conf enabling use_trading_proto_v2
    pub fn seednode_trade_v2(passphrase: &str, coins: &Json, ip_env_var: &str, port_env_var: &str) -> Mm2TestConf {
        Mm2TestConf {
            conf: json!({
                "gui": "nogui",
                "netid": 8999,
                "passphrase": passphrase,
                "coins": coins,
                "rpc_password": DEFAULT_RPC_PASSWORD,
                "i_am_seed": true,
                "use_trading_proto_v2": true,
            }),
            rpc_password: DEFAULT_RPC_PASSWORD.into(),
        }
    }

    /// Generates a seed node conf enabling use_trading_proto_v2
    pub fn seednode_trade_legacy_with_metrics(
        passphrase: &str,
        netid: &usize,
        coins: &Json,
        ip_env_var: &str,
        port_env_var: &str,
    ) -> Mm2TestConf {
        Mm2TestConf {
            conf: json!({
                "gui": "nogui",
                "netid": netid,
                // "dht": "on",
                // "myipaddr": env::var(ip_env_var).ok(),
                // "rpcip": env::var(ip_env_var).ok(),
                // "canbind": env::var(port_env_var).ok().map (|s| s.parse::<i64>().unwrap()),
                "passphrase": passphrase,
                "coins": coins,
                "rpc_password": DEFAULT_RPC_PASSWORD,
                "i_am_seed": true,
                "i_am_metric": true,
                // "use_trading_proto_v2": true,
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

// #[test]
fn test_tendermint_hd_address() {
    let coins = json!([atom_testnet_conf()]);
    // Default address m/44'/118'/0'/0/0 when no path_to_address is specified in activation request
    let expected_address = "cosmos1nv4mqaky7n7rqjhch7829kgypx5s8fh62wdtr8";

    let conf = Mm2TestConf::seednode_with_hd_account(TENDERMINT_TEST_BIP39_SEED, &coins);
    let mm = MarketMakerIt::start(conf.conf, conf.rpc_password, None).unwrap();

    let activation_result = block_on(enable_tendermint(
        &mm,
        ATOM_TICKER,
        &[],
        ATOM_TENDERMINT_RPC_URLS,
        false,
    ));

    let result: RpcV2Response<TendermintActivationResult> = serde_json::from_value(activation_result).unwrap();
    assert_eq!(result.result.address, expected_address);
}

/// Swap ATOM with DOC
#[test]
fn swap_nucleus_with_doc() {
    let bob_passphrase = String::from(BOB_PASSPHRASE);
    let alice_passphrase = String::from(ALICE_PASSPHRASE);

    // let coins = json!([nucleus_testnet_conf(), doc_conf()]);
    let coins = json!([atom_testnet_conf(), doc_conf()]);

    let conf = seednode_trade_legacy_with_metrics(
        // ra format as block
        "atom test seed",
        &9998,
        &coins,
        "BOB_TRADE_IP",
        "BOB_TRADE_PORT",
    );
    let mm_bob = MarketMakerIt::start(
        // json!({
        //     "gui": "nogui",
        //     "netid": 8999,
        //     "dht": "on",
        //     "myipaddr": env::var("BOB_TRADE_IP") .ok(),
        //     "rpcip": env::var("BOB_TRADE_IP") .ok(),
        //     "canbind": env::var("BOB_TRADE_PORT") .ok().map (|s| s.parse::<i64>().unwrap()),
        //     "passphrase": bob_passphrase,
        //     "coins": coins,
        //     "rpc_password": util::DEFAULT_RPC_PASSWORD,
        //     "i_am_seed": true,
        //     // FIXME
        //     "i_am_metric": true,
        // }),
        conf.conf,
        conf.rpc_password,
        None,
    )
    .unwrap();

    thread::sleep(Duration::from_secs(1));

    // normal node
    let mm_alice = MarketMakerIt::start(
        json!({
            "gui": "nogui",
            "netid": 9998,
            "dht": "on",
            "myipaddr": env::var("ALICE_TRADE_IP") .ok(),
            "rpcip": env::var("ALICE_TRADE_IP") .ok(),
            "passphrase": "atom test seed",
            "coins": coins,
            "seednodes": [mm_bob.my_seed_addr()],
            "rpc_password": util::DEFAULT_RPC_PASSWORD,
            "skip_startup_checks": true,
        }),
        util::DEFAULT_RPC_PASSWORD.into(),
        None,
    )
    .unwrap();

    thread::sleep(Duration::from_secs(1));

    // let activation_result = block_on(enable_tendermint(
    //     &mm_bob,
    //     "NUCLEUS-TEST",
    //     &[],
    //     NUCLEUS_TESTNET_RPC_URLS,
    //     false,
    // ));
    let activation_result = block_on(enable_tendermint(
        &mm_bob,
        ATOM_TICKER,
        &[],
        ATOM_TENDERMINT_RPC_URLS,
        false,
    ));
    dbg!(&activation_result);
    let result: RpcV2Response<TendermintActivationResult> = serde_json::from_value(activation_result).unwrap();
    let expected_address = "cosmos1svaw0aqc4584x825ju7ua03g5xtxwd0ahl86hz";
    // assert_eq!(result.result.address, expected_address);

    let zero_balance: BigDecimal = "0.".parse().unwrap();
    assert_ne!(result.result.balance.unwrap().spendable, zero_balance);

    let activation_result = block_on(enable_tendermint(
        &mm_alice,
        ATOM_TICKER,
        &[],
        ATOM_TENDERMINT_RPC_URLS,
        false,
    ));
    dbg!(&activation_result);

    let result: RpcV2Response<TendermintActivationResult> = serde_json::from_value(activation_result).unwrap();
    let expected_address = "cosmos1svaw0aqc4584x825ju7ua03g5xtxwd0ahl86hz";
    // assert_eq!(result.result.address, expected_address);

    let zero_balance: BigDecimal = "0.".parse().unwrap();
    assert_ne!(result.result.balance.unwrap().spendable, zero_balance);

    // dbg!(block_on(enable_electrum(&mm_bob, "DOC", false, DOC_ELECTRUM_ADDRS)));
    // dbg!(block_on(enable_electrum(&mm_alice, "DOC", false, DOC_ELECTRUM_ADDRS)));

    dbg!(block_on(enable_electrum_json(
        &mm_bob,
        "DOC",
        false,
        util::get_doc_electrum_addrs().clone(),
    )));
    dbg!(block_on(enable_electrum_json(
        &mm_alice,
        "DOC",
        false,
        util::get_doc_electrum_addrs(),
    )));

    block_on(trade_base_rel_tendermint(
        mm_bob,
        mm_alice,
        ATOM_TICKER,
        "DOC",
        1,
        2,
        0.008,
    ));
}

// #[test]
// fn swap_doc_with_nucleus() {
//     let bob_passphrase = String::from(BOB_PASSPHRASE);
//     let alice_passphrase = String::from(ALICE_PASSPHRASE);

//     let coins = json!([nucleus_testnet_conf(), doc_conf()]);

//     let mm_bob = MarketMakerIt::start(
//         json!({
//             "gui": "nogui",
//             "netid": 8999,
//             "dht": "on",
//             "myipaddr": env::var("BOB_TRADE_IP") .ok(),
//             "rpcip": env::var("BOB_TRADE_IP") .ok(),
//             "canbind": env::var("BOB_TRADE_PORT") .ok().map (|s| s.parse::<i64>().unwrap()),
//             "passphrase": bob_passphrase,
//             "coins": coins,
//             "rpc_password": "password",
//             "i_am_seed": true,
//         }),
//         "password".into(),
//         None,
//     )
//     .unwrap();

//     thread::sleep(Duration::from_secs(1));

//     let mm_alice = MarketMakerIt::start(
//         json!({
//             "gui": "nogui",
//             "netid": 8999,
//             "dht": "on",
//             "myipaddr": env::var("ALICE_TRADE_IP") .ok(),
//             "rpcip": env::var("ALICE_TRADE_IP") .ok(),
//             "passphrase": alice_passphrase,
//             "coins": coins,
//             "seednodes": [mm_bob.my_seed_addr()],
//             "rpc_password": "password",
//             "skip_startup_checks": true,
//         }),
//         "password".into(),
//         None,
//     )
//     .unwrap();

//     thread::sleep(Duration::from_secs(1));

//     dbg!(block_on(enable_tendermint(
//         &mm_bob,
//         "NUCLEUS-TEST",
//         &[],
//         NUCLEUS_TESTNET_RPC_URLS,
//         false
//     )));

//     dbg!(block_on(enable_tendermint(
//         &mm_alice,
//         "NUCLEUS-TEST",
//         &[],
//         NUCLEUS_TESTNET_RPC_URLS,
//         false
//     )));

//     dbg!(block_on(enable_electrum(&mm_bob, "DOC", false, DOC_ELECTRUM_ADDRS)));

//     dbg!(block_on(enable_electrum(&mm_alice, "DOC", false, DOC_ELECTRUM_ADDRS)));

//     block_on(trade_base_rel_tendermint(
//         mm_bob,
//         mm_alice,
//         "DOC",
//         "NUCLEUS-TEST",
//         1,
//         2,
//         0.008,
//     ));
// }

// Ensure seednode startup with metrics is detected
// let expected_log = format!("lp_init: Seednode startup with metrics detected");
// mm_bob
//     .wait_for_log(5., |log| log.contains(&expected_log))
//     .await
//     .unwrap();

// panic!("test");

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
