use crate::integration_tests_common::{enable_electrum, enable_electrum_json};
use crate::mm2_tests::tendermint_tests::swap::*;
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

const BOB_PASSPHRASE: &str = "test seed";
const ALICE_PASSPHRASE: &str = "test2 seed";

mod util {
    use super::*;

    /// Due to `enable_electrum`, far all intents as purposes, being marked as deprecated
    pub(crate) fn get_doc_electrum_addrs() -> Vec<Json> {
        DOC_ELECTRUM_ADDRS
            .iter()
            .map(|elm| json::from_str(*elm).expect("!Value"))
            .collect()
    }
}

#[test]
fn swap_nucleus_with_doc_and_seednode_metrics() {
    let bob_passphrase = String::from(BOB_PASSPHRASE);
    let alice_passphrase = String::from(ALICE_PASSPHRASE);

    let coins = json!([nucleus_testnet_conf(), doc_conf()]);

    let mm_bob_seed = MarketMakerIt::start(
        json!({
            "gui": "nogui",
            "netid": 8999,
            "dht": "on",
            "myipaddr": env::var("BOB_TRADE_IP") .ok(),
            "rpcip": env::var("BOB_TRADE_IP") .ok(),
            "canbind": env::var("BOB_TRADE_PORT") .ok().map (|s| s.parse::<i64>().unwrap()),
            "passphrase": bob_passphrase,
            "coins": coins,
            "rpc_password": "password",
            "i_am_seed": true,
            "i_am_metric": true,
        }),
        "password".into(),
        None,
    )
    .unwrap();

    thread::sleep(Duration::from_secs(1));

    let mm_alice = MarketMakerIt::start(
        json!({
            "gui": "nogui",
            "netid": 8999,
            "dht": "on",
            "myipaddr": env::var("ALICE_TRADE_IP") .ok(),
            "rpcip": env::var("ALICE_TRADE_IP") .ok(),
            "passphrase": alice_passphrase,
            "coins": coins,
            "seednodes": [mm_bob_seed.my_seed_addr()],
            "rpc_password": "password",
            "skip_startup_checks": true,
        }),
        "password".into(),
        None,
    )
    .unwrap();

    thread::sleep(Duration::from_secs(1));

    dbg!(block_on(enable_tendermint(
        &mm_bob_seed,
        "NUCLEUS-TEST",
        &[],
        NUCLEUS_TESTNET_RPC_URLS,
        false
    )));

    dbg!(block_on(enable_tendermint(
        &mm_alice,
        "NUCLEUS-TEST",
        &[],
        NUCLEUS_TESTNET_RPC_URLS,
        false
    )));

    dbg!(block_on(enable_electrum_json(
        &mm_bob_seed,
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

    block_on(trade_base_rel_tendermint_with_metrics(
        mm_bob_seed,
        mm_alice,
        "NUCLEUS-TEST",
        "DOC",
        1,
        2,
        0.008,
    ));
}

pub async fn trade_base_rel_tendermint_with_metrics(
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
