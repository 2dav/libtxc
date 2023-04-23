include!("common/common.rs");

use std::time::Instant;

use libtxc::{LogLevel, Stream, TransaqConnector};
use tracing::info;

// запуск примера:
// cargo run --release --example bench --no-default-features
//
// Попробуем оценить накладные расходы на круг(отправка-получение) связанные с реализацией коннектора
#[allow(non_upper_case_globals)]
fn main() -> anyhow::Result<()> {
    let (_, _, lib, logdir) = init()?;
    init_logging();

    let mut txc = TransaqConnector::new(lib.into(), logdir.into(), LogLevel::Minimum)?;
    let sender = txc.sender();

    const N: usize = 20000;
    static mut deltas: [usize; N] = [0; N];
    static mut send_delta: [usize; N] = [0; N];

    let mut i = 0;
    let mut prev = Instant::now();

    txc.input_stream().subscribe(move |_| {
        let now = Instant::now();

        let delta = (now - std::mem::replace(&mut prev, now)).as_micros();
        unsafe { *deltas.as_mut_ptr().add(i) = delta as usize };
        i += 1;
    });

    let get_version = "<command id = \"get_connector_version\"/>\0";

    unsafe {
        for i in 0..N {
            let start = Instant::now();
            sender.send(get_version)?;
            *send_delta.as_mut_ptr().add(i) = (Instant::now() - start).as_micros() as usize;
        }

        macro_rules! mean {
            ($x:expr) => {
                $x.iter().cloned().sum::<usize>() as f64 / ($x.len() as f64)
            };
        }

        let mean = mean!(deltas[1..]);
        let var = deltas[1..].iter().cloned().map(|x| (mean - x as f64).powi(2)).sum::<f64>()
            / ((N - 2) as f64);
        let sdelta = mean!(send_delta);
        info!("mean: {mean:.2} us, std: {:.2} us, send_time:{} us", var.sqrt(), sdelta);
    }

    Ok(())
}
