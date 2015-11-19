#[macro_use] extern crate nickel;
extern crate env_logger;

use std::collections::HashMap;
use nickel::Nickel;

fn main() {
    env_logger::init().unwrap();
    let mut server = Nickel::new();

    server.utilize(router! {
        get "/" => |_req, res| {
            let mut data = HashMap::new();
            data.insert("name", "Sune");
            return res.render("templates/index.tpl", &data);
        }
    });

    server.listen("127.0.0.1:6767");
}
