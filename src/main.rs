fn main() {
    println!("{}", greeting());
}

fn greeting() -> &'static str {
    "Is Retiro open?"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn greeting_names_retiro() {
        assert_eq!(greeting(), "Is Retiro open?");
    }
}
