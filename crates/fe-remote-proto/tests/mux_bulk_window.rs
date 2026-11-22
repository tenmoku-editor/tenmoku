//! The bulk window (D § 2.4): `2 × delivery rate × min RTT`, clamped to
//! 256 KiB–4 MiB and resized at each heartbeat, checked on the `Delayed`
//! link with a bandwidth cap (D § 14 asks phase 2 to validate it there).
//!
//! These run on real time, as the latency they measure is real. The
//! heartbeat is shortened so a run takes seconds, and every bound leaves
//! room for a loaded machine.

use fe_remote_proto::testing::{self, Latency};
use fe_remote_proto::window::{
    DeliveryRate, INITIAL_WINDOW, MAX_BULK_WINDOW, MIN_WINDOW, bulk_window,
};
use fe_remote_proto::{
    Config, ControlOp, Event, Link, Mux, OpenTarget, Opened, Status, Stream, chan,
};
use std::thread;
use std::time::{Duration, Instant};

const MIB: f64 = (1 << 20) as f64;

fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}

#[test]
fn the_bulk_window_is_twice_rate_times_min_rtt_clamped() {
    assert_eq!((MIN_WINDOW, MAX_BULK_WINDOW), (256 << 10, 4 << 20));
    assert_eq!(bulk_window(10.0 * MIB, ms(50)), 1 << 20);
    assert_eq!(bulk_window(4.0 * MIB, ms(100)), 838_860);
    // A slow link, or a slow consumer, gets the floor ...
    assert_eq!(bulk_window(MIB, ms(50)), MIN_WINDOW);
    assert_eq!(bulk_window(0.0, ms(50)), MIN_WINDOW);
    assert_eq!(bulk_window(f64::NAN, ms(50)), MIN_WINDOW);
    // ... and a fast one the ceiling.
    assert_eq!(bulk_window(1024.0 * MIB, ms(50)), MAX_BULK_WINDOW);
    assert_eq!(bulk_window(f64::INFINITY, ms(50)), MAX_BULK_WINDOW);
}

#[test]
fn the_delivery_rate_is_the_most_of_the_last_three_intervals_with_data() {
    let mut rate = DeliveryRate::default();
    assert_eq!(rate.rate(), None);
    // An idle interval is no sample.
    assert_eq!(rate.sample(0, ms(500)), None);
    assert_eq!(rate.sample(10, Duration::from_secs(1)), Some(10.0));
    assert_eq!(rate.sample(30, Duration::from_secs(1)), Some(30.0));
    assert_eq!(rate.sample(20, Duration::from_secs(1)), Some(30.0));
    assert_eq!(rate.sample(5, Duration::from_secs(1)), Some(30.0));
    assert_eq!(rate.sample(5, Duration::from_secs(1)), Some(20.0));
    assert_eq!(rate.sample(0, Duration::from_secs(1)), Some(20.0));
    assert_eq!(rate.sample(5, Duration::from_secs(1)), Some(5.0));
}

/// A client over `latency` to a server running the test service, pinging
/// every `heartbeat`.
fn client(latency: Latency, heartbeat: Duration) -> Mux {
    let (ours, theirs) = Link::pipe().unwrap();
    thread::spawn(move || {
        let (mux, _) = Mux::server(theirs, Config::default(), testing::accept_any).unwrap();
        testing::serve(&mux);
    });
    let config = Config {
        heartbeat: Some(heartbeat),
        ..Config::default()
    };
    let ours = testing::delayed(ours, latency);
    Mux::client(ours, config, testing::location(), None)
        .unwrap()
        .0
}

/// Opens a stream the server floods, marked bulk on this side if `bulk`.
fn flood(client: &Mux, bulk: bool) -> Stream {
    let mut open = client
        .request(
            chan::CONTROL,
            &ControlOp::Open(OpenTarget::Tcp {
                host: "bulk".into(),
                port: 0,
            }),
        )
        .unwrap();
    let Some(Event::Data(body)) = open.recv() else {
        panic!("no channel");
    };
    let opened: Opened = open.decode(&body).unwrap();
    assert_eq!(open.recv(), Some(Event::End(Status::Ok)));
    client.set_bulk(opened.chan, bulk);
    client.stream(opened.chan).unwrap()
}

/// What [`drink`] took.
struct Drunk {
    /// Everything taken since the stream opened, and how long that took.
    total: usize,
    elapsed: Duration,
    /// The bytes taken in the last `tail`.
    tail: usize,
}

/// Takes what the stream delivers for `run`, counting the last `tail`
/// apart.
fn drink(s: &mut Stream, run: Duration, tail: Duration) -> Drunk {
    let t = Instant::now();
    let mut total = 0;
    let mut tail_bytes = 0;
    while t.elapsed() < run {
        let Some(Event::Data(b)) = s.recv() else {
            panic!("the stream ended");
        };
        total += b.len();
        if t.elapsed() >= run - tail {
            tail_bytes += b.len();
        }
    }
    Drunk {
        total,
        elapsed: t.elapsed(),
        tail: tail_bytes,
    }
}

/// Drinks from two streams at once, on two threads started together, so
/// a loaded host slows both alike and only their ratio is meaningful.
fn drink_both(a: &mut Stream, b: &mut Stream, run: Duration, tail: Duration) -> (Drunk, Drunk) {
    let go = std::sync::Barrier::new(2);
    thread::scope(|scope| {
        let a = scope.spawn(|| {
            go.wait();
            drink(a, run, tail)
        });
        let b = scope.spawn(|| {
            go.wait();
            drink(b, run, tail)
        });
        (a.join().unwrap(), b.join().unwrap())
    })
}

/// The wire releases bytes no faster than its cap from the moment the
/// stream opened, whatever the host's load: a slow reader only takes
/// them later. So the total taken can't beat `cap × elapsed` (plus the
/// 0.3 MiB the link holds in flight), unlike a rate over a short span,
/// where a reader catching up takes a burst.
fn assert_within_cap(d: &Drunk, cap: f64) {
    let ceiling = cap * d.elapsed.as_secs_f64() + MIB;
    assert!(
        (d.total as f64) <= ceiling,
        "took {:.1} MiB in {:?}, over a {:.0} MiB/s cap",
        d.total as f64 / MIB,
        d.elapsed,
        cap / MIB
    );
}

fn capped(cap: f64) -> Latency {
    Latency {
        one_way: ms(20),
        bytes_per_sec: Some(cap),
    }
}

#[test]
fn a_capped_link_delivers_at_its_cap() {
    // Not bulk: the 1.25 MiB window is over the link's 0.3 MiB in flight,
    // so the cap is all that limits the stream. The wire releases bytes
    // at its cap whatever the host's load, so what is checked is that it
    // never releases more (a lower bound on the rate would be the host's,
    // not the link's; the bulk test below compares a stream's rate with
    // this one's under the same load).
    let cap = 8.0 * MIB;
    let client = client(capped(cap), Duration::from_secs(10));
    let mut s = flood(&client, false);
    let d = drink(&mut s, Duration::from_secs(3), Duration::from_secs(2));
    assert!(d.total > 0, "nothing arrived");
    assert_within_cap(&d, cap);
}

#[test]
fn a_capped_link_sizes_a_bulk_window_to_twice_its_bdp_and_stays_full() {
    let cap = 8.0 * MIB;
    let client = client(capped(cap), ms(500));
    // The reference: the same link, not bulk, whose 1.25 MiB window is
    // over its bandwidth-delay product, so it keeps the link full. Set up
    // first, so the bulk stream's window is read as it opens, before any
    // of its data has moved the estimate.
    let reference_client = self::client(capped(cap), Duration::from_secs(10));
    let mut reference = flood(&reference_client, false);
    let mut s = flood(&client, true);
    assert_eq!(client.window(s.chan()), INITIAL_WINDOW);
    let (bulk, full) = drink_both(
        &mut s,
        &mut reference,
        Duration::from_secs(5),
        Duration::from_secs(2),
    );
    let rtt = client.rtt().expect("no pong").min;
    assert!(rtt >= ms(40), "{rtt:?}");
    // 2 × 8 MiB/s × ~40 ms ≈ 0.66 MiB: well under the initial window, so
    // the estimate moved it.
    let expected = f64::from(bulk_window(cap, rtt));
    let window = f64::from(client.window(s.chan()));
    assert!(
        window > 0.5 * expected && window < 1.25 * expected,
        "window {:.2} MiB, expected {:.2} MiB at {rtt:?}",
        window / MIB,
        expected / MIB
    );
    // Twice the BDP still keeps the link busy: over the same span, under
    // the same load, the bulk stream takes what the stream that can't be
    // window-limited does. A window that starved the link takes less.
    assert_within_cap(&bulk, cap);
    assert_within_cap(&full, cap);
    let ratio = bulk.tail as f64 / full.tail as f64;
    assert!(ratio > 0.8, "bulk took {ratio:.2} of a full link");
}

#[test]
fn an_uncapped_link_grows_a_bulk_window_to_its_ceiling() {
    let client = client(Latency::one_way(ms(50)), ms(500));
    let mut s = flood(&client, true);
    assert_eq!(client.window(s.chan()), INITIAL_WINDOW);
    // Held to its window, the stream delivers a window a round trip, so
    // each heartbeat's estimate is about twice the last.
    let t = Instant::now();
    while client.window(s.chan()) < MAX_BULK_WINDOW {
        assert!(
            t.elapsed() < Duration::from_secs(30),
            "the window stopped at {}",
            client.window(s.chan())
        );
        assert!(matches!(s.recv(), Some(Event::Data(_))));
    }
}

#[test]
fn a_channel_that_is_not_bulk_keeps_its_window() {
    let client = client(Latency::one_way(ms(50)), ms(200));
    let mut s = flood(&client, false);
    drink(&mut s, Duration::from_secs(2), Duration::ZERO);
    assert_eq!(client.window(s.chan()), INITIAL_WINDOW);
}

#[test]
fn a_bulk_window_turned_off_goes_back_to_the_initial_window() {
    let client = client(Latency::one_way(ms(50)), ms(200));
    let mut s = flood(&client, true);
    let t = Instant::now();
    while client.window(s.chan()) == INITIAL_WINDOW {
        assert!(t.elapsed() < Duration::from_secs(30), "never resized");
        assert!(matches!(s.recv(), Some(Event::Data(_))));
    }
    client.set_bulk(s.chan(), false);
    assert_eq!(client.window(s.chan()), INITIAL_WINDOW);
    // The stream keeps flowing on the restored window.
    drink(&mut s, ms(500), Duration::ZERO);
}
