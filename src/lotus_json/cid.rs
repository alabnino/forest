use super::*;

pub type CidLotusJson = CidLotusJsonGeneric<64>;

#[derive(Serialize, Deserialize, From, Into)]
pub struct CidLotusJsonGeneric<const S: usize> {
    #[serde(rename = "/", with = "stringify")]
    slash: ::cid::CidGeneric<S>,
}

impl<const S: usize> HasLotusJson for ::cid::CidGeneric<S> {
    type LotusJson = CidLotusJsonGeneric<S>;
}

#[test]
fn test() {
    assert_snapshot(json!({"/": "baeaaaaa"}), ::cid::Cid::default());
}

#[cfg(test)]
quickcheck! {
    fn round_trip(val: ::cid::Cid) -> bool {
        assert_via_json(val);
        true
    }
}
