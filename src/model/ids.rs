pub type ShortId = String;

pub fn new_id() -> ShortId {
    uuid::Uuid::new_v4().to_string()[..8].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_is_8_chars() {
        let id = new_id();
        assert_eq!(id.len(), 8);
    }

    #[test]
    fn ids_are_unique() {
        let a = new_id();
        let b = new_id();
        assert_ne!(a, b);
    }
}
