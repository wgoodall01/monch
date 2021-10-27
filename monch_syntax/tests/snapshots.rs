use insta::{assert_yaml_snapshot, glob};
use monch_syntax::Parser;
use std::fs;

fn display_err<E: std::error::Error>(err: E) -> E {
    println!("Error:");
    println!("{}", err);
    println!("{:#?}", err);
    err
}

#[test]
fn snapshot_script() {
    glob!("fixtures/*.example.monch", |path| {
        let input = fs::read_to_string(path).unwrap();

        // Make a parser
        let parsed = Parser::new()
            .parse_script(&input)
            .map_err(display_err)
            .expect("Script failed to parse");

        assert_yaml_snapshot!(parsed);
    })
}
