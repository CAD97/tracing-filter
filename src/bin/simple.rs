use tracing_filter::simple::Filter;

fn main() {
    let mut args = std::env::args();
    match (args.next(), args.next(), args.next()) {
        (_, Some(directive), None) => show_directive(directive),
        _ => eprintln!("Usage: cargo run --features miette/fancy --bin simple <directive>"),
    }
}

fn show_directive(spec: String) {
    let (filter, report) = Filter::parse(spec);

    println!();

    if let Some(filter) = filter {
        println!("Filter normalizes as:\n{filter}\n");
    }

    if let Some(report) = report {
        println!("Error: {report:?}");
    }
}
