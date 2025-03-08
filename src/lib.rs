pub mod render;
pub mod command;
pub mod dom;
pub mod html_parser;
pub mod css;
pub mod css_parser;
pub mod style;
pub mod layout;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
