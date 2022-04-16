use tracing_filter::legacy::Filter;

fn main() {
    let mut args = std::env::args();
    match (args.next(), args.next(), args.next()) {
        (_, Some(directive), None) => show_directive(&directive),
        _ => eprintln!("Usage: cargo run --features miette/fancy --bin legacy <directive>"),
    }
}

fn show_directive(spec: &str) {
    let (filter, report) = Filter::parse(spec);

    println!("\nFilter normalizes as:\n{}\n", filter);
    if let Some(report) = report {
        println!("Error: {:?}", report);
    }
}
