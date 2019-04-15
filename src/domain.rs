use wx::domain::HazardType;
use wx::error::{Error, WxError};

#[derive(Deserialize, Eq, PartialEq, Serialize, Clone)]
pub enum Hazard {
    Tornado = 0isize,
    Funnel,
    WallCloud,
    Hail,
    Wind,
    Flood,
    FlashFlood,
    Other,
    FreezingRain,
    Snow,
}

impl Hazard {
    pub fn get_by_code(code: &str) -> Result<Hazard, Error> {
        match code {
            "1" => Ok(Hazard::Tornado),
            "2" => Ok(Hazard::Funnel),
            "3" => Ok(Hazard::WallCloud),
            "4" => Ok(Hazard::Hail),
            "5" => Ok(Hazard::Wind),
            "6" => Ok(Hazard::Flood),
            "7" => Ok(Hazard::FlashFlood),
            "8" => Ok(Hazard::Other),
            "9" => Ok(Hazard::FreezingRain),
            "10" => Ok(Hazard::Snow),
            _ => {
                let reason = format!("unknown code: {}", code.to_string());
                Err(Error::Wx(<WxError>::new(&reason)))
            }
        }
    }

    pub fn to_hazard_type(&self) -> HazardType {
        match self {
            Hazard::Tornado => HazardType::Tornado,
            Hazard::Funnel => HazardType::Funnel,
            Hazard::WallCloud => HazardType::WallCloud,
            Hazard::Hail => HazardType::Hail,
            Hazard::Wind => HazardType::Wind,
            Hazard::Flood => HazardType::Flood,
            Hazard::FlashFlood => HazardType::Flood,
            Hazard::Other => HazardType::Other {
                kind: "SN Other".to_string(),
            },
            Hazard::FreezingRain => HazardType::FreezingRain,
            Hazard::Snow => HazardType::Snow,
        }
    }

    pub fn to_string(&self) -> String {
        match self {
            Hazard::Tornado => "Tornado",
            Hazard::Funnel => "Funnel",
            Hazard::WallCloud => "Wall Cloud",
            Hazard::Hail => "Hail",
            Hazard::Wind => "Wind",
            Hazard::Flood => "Flood",
            Hazard::FlashFlood => "Flash Flood",
            Hazard::Other => "Other",
            Hazard::FreezingRain => "Freezing Rain",
            Hazard::Snow => "Snow",
        }
        .to_string()
    }
}
