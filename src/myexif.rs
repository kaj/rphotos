//! Extract all the exif data I care about
use adm::result::Error;
use chrono::{Date, Local, NaiveDate, NaiveDateTime, Utc};
use exif::{Field, Reader, Tag, Value};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::str::from_utf8;

#[derive(Debug, Default)]
pub struct ExifData {
    dateval: Option<NaiveDateTime>,
    gpsdate: Option<Date<Utc>>,
    gpstime: Option<(u8, u8, u8)>,
    make: Option<String>,
    model: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    orientation: Option<u32>,
    latval: Option<f64>,
    longval: Option<f64>,
    latref: Option<String>,
    longref: Option<String>,
}

impl ExifData {
    pub fn read_from(path: &Path) -> Result<Self, Error> {
        let mut result = Self::default();
        let file = File::open(path)?;
        let reader = Reader::new(&mut BufReader::new(&file))?;
        for f in reader.fields() {
            if !f.thumbnail {
                if let Some(d) = is_datetime(f, &Tag::DateTimeOriginal) {
                    result.dateval = Some(d);
                } else if let Some(d) = is_datetime(f, &Tag::DateTime) {
                    result.dateval = Some(d);
                } else if let Some(d) = is_datetime(f, &Tag::DateTimeDigitized)
                {
                    if result.dateval.is_none() {
                        result.dateval = Some(d)
                    }
                } else if let Some(s) = is_string(f, &Tag::Make) {
                    result.make = Some(s.to_string());
                } else if let Some(s) = is_string(f, &Tag::Model) {
                    result.model = Some(s.to_string());
                } else if let Some(w) = is_u32(f, &Tag::PixelXDimension) {
                    result.width = Some(w);
                } else if let Some(h) = is_u32(f, &Tag::PixelYDimension) {
                    result.height = Some(h);
                } else if let Some(w) = is_u32(f, &Tag::ImageWidth) {
                    result.width = Some(w);
                } else if let Some(h) = is_u32(f, &Tag::ImageLength) {
                    result.height = Some(h);
                } else if let Some(o) = is_u32(f, &Tag::Orientation) {
                    result.orientation = Some(o);
                } else if let Some(lat) = is_lat_long(f, &Tag::GPSLatitude) {
                    result.latval = Some(lat);
                } else if let Some(long) = is_lat_long(f, &Tag::GPSLongitude) {
                    result.longval = Some(long);
                } else if let Some(s) = is_string(f, &Tag::GPSLatitudeRef) {
                    result.latref = Some(s.to_string());
                } else if let Some(s) = is_string(f, &Tag::GPSLongitudeRef) {
                    result.longref = Some(s.to_string());
                } else if let Some(s) = is_string(f, &Tag::GPSImgDirectionRef)
                {
                    println!("  direction ref: {}", s);
                } else if let Some(s) = is_string(f, &Tag::GPSImgDirection) {
                    println!("  direction: {}", s);
                } else if let Some(d) = is_date(f, &Tag::GPSDateStamp) {
                    result.gpsdate = Some(d);
                } else if let Some(hms) = is_time(f, &Tag::GPSTimeStamp) {
                    result.gpstime = Some(hms);
                }
            }
            //println!("    {} ({}) {:?}", f.tag, f.thumbnail, f.value);
        }
        Ok(result)
    }

    pub fn date(&self) -> Option<NaiveDateTime> {
        // Note: I probably return and store datetime with tz,
        // possibly utc, instead.
        if let (&Some(date), &Some((h, m, s))) = (&self.gpsdate, &self.gpstime)
        {
            let naive = date.and_hms(h as u32, m as u32, s as u32)
                .with_timezone(&Local)
                .naive_local();
            debug!("GPS Date {}, {}:{}:{} => {}", date, h, m, s, naive);
            Some(naive)
        } else if let Some(date) = self.dateval {
            Some(date)
        } else {
            warn!("No date found in exif");
            None
        }
    }
    pub fn camera(&self) -> Option<(&str, &str)> {
        if let (&Some(ref make), &Some(ref model)) = (&self.make, &self.model)
        {
            Some((make, model))
        } else {
            None
        }
    }
    pub fn position(&self) -> Option<(f64, f64)> {
        if let (Some(lat), Some(long)) = (self.lat(), self.long()) {
            Some((lat, long))
        } else {
            None
        }
    }
    fn lat(&self) -> Option<f64> {
        match (&self.latref, self.latval) {
            (&Some(ref r), Some(lat)) if r == "N" => Some(lat.abs()),
            (&Some(ref r), Some(lat)) if r == "S" => Some(-(lat.abs())),
            (&Some(ref r), lat) => {
                error!("Bad latref: {}", r);
                lat
            }
            (&None, lat) => lat,
        }
    }
    fn long(&self) -> Option<f64> {
        match (&self.longref, self.longval) {
            (&Some(ref r), Some(long)) if r == "E" => Some(long.abs()),
            (&Some(ref r), Some(long)) if r == "W" => Some(-(long.abs())),
            (&Some(ref r), long) => {
                error!("Bad longref: {}", r);
                long
            }
            (&None, long) => long,
        }
    }

    pub fn rotation(&self) -> Result<i16, Error> {
        if let Some(value) = self.orientation {
            debug!("Raw orientation is {}", value);
            match value {
                1 | 0 => Ok(0),
                3 => Ok(180),
                6 => Ok(90),
                8 => Ok(270),
                x => Err(Error::UnknownOrientation(x)),
            }
        } else {
            debug!("Orientation tag missing, default to 0 degrees");
            Ok(0)
        }
    }
}

fn is_lat_long(f: &Field, tag: &Tag) -> Option<f64> {
    if f.tag == *tag {
        match &f.value {
            &Value::Rational(ref v) if v.len() == 3 => {
                let (v0, v1, v2) =
                    (v[0].to_f64(), v[1].to_f64(), v[2].to_f64());
                return Some(v0 + (v1 + v2 / 60.0) / 60.0);
            }
            ref v => {
                println!("ERROR: Bad value for {}: {:?}", tag, v);
                None
            }
        }
    } else {
        None
    }
}

fn is_datetime(f: &Field, tag: &Tag) -> Option<NaiveDateTime> {
    if f.tag == *tag {
        match single_datetime(&f.value) {
            Ok(date) => Some(date),
            Err(err) => {
                println!("ERROR: Expected datetime for {}: {:?}", tag, err);
                None
            }
        }
    } else {
        None
    }
}

fn is_date(f: &Field, tag: &Tag) -> Option<Date<Utc>> {
    if f.tag == *tag {
        match single_date(&f.value) {
            Ok(date) => Some(date),
            Err(err) => {
                println!("ERROR: Expected date for {}: {:?}", tag, err);
                None
            }
        }
    } else {
        None
    }
}

fn is_time(f: &Field, tag: &Tag) -> Option<(u8, u8, u8)> {
    if f.tag == *tag {
        match &f.value {
            &Value::Rational(ref v)
                if v.len() == 3 && v[0].denom == 1 && v[1].denom == 1
                    && v[2].denom == 1 =>
            {
                Some((v[0].num as u8, v[1].num as u8, v[2].num as u8))
            }
            err => {
                error!("Expected time for {}: {:?}", tag, err);
                None
            }
        }
    } else {
        None
    }
}

fn is_string<'a>(f: &'a Field, tag: &Tag) -> Option<&'a str> {
    if f.tag == *tag {
        match single_ascii(&f.value) {
            Ok(s) => Some(s),
            Err(err) => {
                println!("ERROR: Expected string for {}: {:?}", tag, err);
                None
            }
        }
    } else {
        None
    }
}

fn is_u32(f: &Field, tag: &Tag) -> Option<u32> {
    if f.tag == *tag {
        match &f.value {
            &Value::Long(ref v) if v.len() == 1 => Some(v[0]),
            &Value::Short(ref v) if v.len() == 1 => Some(v[0] as u32),
            v => {
                println!("ERROR: Unsuppored value for {}: {:?}", tag, v);
                None
            }
        }
    } else {
        None
    }
}

fn single_datetime(value: &Value) -> Result<NaiveDateTime, Error> {
    single_ascii(value)
        .and_then(|s| Ok(NaiveDateTime::parse_from_str(s, "%Y:%m:%d %T")?))
}

fn single_date(value: &Value) -> Result<Date<Utc>, Error> {
    single_ascii(value).and_then(|s| {
        Ok(Date::from_utc(
            NaiveDate::parse_from_str(s, "%Y:%m:%d")?,
            Utc,
        ))
    })
}

fn single_ascii<'a>(value: &'a Value) -> Result<&'a str, Error> {
    match value {
        &Value::Ascii(ref v) if v.len() == 1 => Ok(from_utf8(v[0])?),
        &Value::Ascii(ref v) if v.len() > 1 => {
            for t in &v[1..] {
                if !t.is_empty() {
                    return Err(Error::Other(format!(
                        "Got {:?}, expected single ascii value",
                        v,
                    )));
                }
            }
            Ok(from_utf8(v[0])?)
        }
        v => Err(Error::Other(format!(
            "Got {:?}, expected single ascii value",
            v,
        ))),
    }
}
