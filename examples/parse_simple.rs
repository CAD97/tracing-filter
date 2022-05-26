use tracing_filter::simple::Filter;

fn main() {
    let mut args = std::env::args();
    match (args.next(), args.next(), args.next()) {
        (_, Some(directive), None) => show_directive(&directive),
        _ => eprintln!("Usage: cargo run --example parse_simple -- <directive>"),
    }
}

fn show_directive(spec: &str) {
    let (filter, report) = Filter::parse(spec);

    println!();

    if let Some(filter) = filter {
        println!("Filter normalizes as:\n{}\n", filter);
    }

    if let Some(report) = report {
        println!("Error: {:?}", report);
    }

    let mut builder = env_logger::Builder::new();
    builder.parse_filters(spec);
    println!("\nUpstream interprests it as:\n{:#?}\n", builder.build());
}
