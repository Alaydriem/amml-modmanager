use amm_bench::fixtures::{ComposeMode, ComposeRig, RouteLatency};
use amm_core::structs::descriptor::Capability::{Hud, Overlay};

const NAME: &str = "route_during_compose";
const SAMPLES: usize = 10_000;

fn main() {
    // Cargo passes `--bench`; any other argument that is not a flag narrows the run by substring.
    let filters: Vec<String> = std::env::args().skip(1).filter(|arg| !arg.starts_with('-')).collect();
    if !filters.is_empty() && !filters.iter().any(|filter| NAME.contains(filter.as_str())) {
        return;
    }

    let baseline = ComposeRig::idle();
    println!("{}", RouteLatency::run(&baseline, ComposeMode::None, SAMPLES).line(NAME, "no_client", "baseline"));
    drop(baseline);

    let one = ComposeRig::with_clients(&[&[Overlay]]);
    println!(
        "{}",
        RouteLatency::run(&one, ComposeMode::None, SAMPLES).line(NAME, "one_overlay_client", "baseline_no_compose")
    );
    println!("{}", RouteLatency::run(&one, ComposeMode::Paced, SAMPLES).line(NAME, "one_overlay_client", "paced_60hz"));
    println!(
        "{}",
        RouteLatency::run(&one, ComposeMode::BackToBack, SAMPLES).line(NAME, "one_overlay_client", "back_to_back")
    );
    drop(one);

    let two = ComposeRig::with_clients(&[&[Hud, Overlay], &[Overlay]]);
    println!(
        "{}",
        RouteLatency::run(&two, ComposeMode::Paced, SAMPLES).line(NAME, "two_clients_3_layers", "paced_60hz")
    );
    println!(
        "{}",
        RouteLatency::run(&two, ComposeMode::BackToBack, SAMPLES).line(NAME, "two_clients_3_layers", "back_to_back")
    );
}
