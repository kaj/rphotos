//! Extract all the exif data I care about
use crate::adm::result::Error;
use chrono::{Local, NaiveDate, NaiveDateTime, NaiveTime, TimeZone};
use exif::{Field, In, Reader, Tag, Value};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::str::from_utf8;
use tracing::{debug, error, warn};

#[derive(Debug, Default)]
pub struct ExifData {
    dateval: Option<NaiveDateTime>,
    /// Combines with gpstime to a datetime which is Utc.
    gpsdate: Option<NaiveDate>,
    /// Combines with gpstime to a datetime which is Utc.
    gpstime: Option<NaiveTime>,
    make: Option<String>,
    model: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    orientation: Option<u32>,
    latval: Option<f64>,
    longval: Option<f64>,
    latref: Option<String>,
    longref: Option<String>,
}

impl ExifData {
    pub fn read_from(path: &Path) -> Result<Self, Error> {
        let mut result = Self::default();
        let file = File::open(path).map_err(|e| Error::in_file(&e, path))?;
        let reader = Reader::new()
            .read_from_container(&mut BufReader::new(&file))
            .map_err(|e| Error::in_file(&e, path))?;
        for f in reader.fields() {
            if f.ifd_num == In::PRIMARY {
                if let Some(d) = is_datetime(f, Tag::DateTimeOriginal) {
                    result.dateval = Some(d);
                } else if let Some(d) = is_datetime(f, Tag::DateTime) {
                    result.dateval = Some(d);
                } else if let Some(d) = is_datetime(f, Tag::DateTimeDigitized)
                {
                    if result.dateval.is_none() {
                        result.dateval = Some(d)
                    }
                } else if let Some(s) = is_string(f, Tag::Make) {
                    result.make = Some(s.to_string());
                } else if let Some(s) = is_string(f, Tag::Model) {
                    result.model = Some(s.to_string());
                } else if let Some(w) = is_u32(f, Tag::PixelXDimension) {
                    result.width = Some(w);
                } else if let Some(h) = is_u32(f, Tag::PixelYDimension) {
                    result.height = Some(h);
                } else if let Some(w) = is_u32(f, Tag::ImageWidth) {
                    result.width = Some(w);
                } else if let Some(h) = is_u32(f, Tag::ImageLength) {
                    result.height = Some(h);
                } else if let Some(o) = is_u32(f, Tag::Orientation) {
                    result.orientation = Some(o);
                } else if let Some(lat) = is_lat_long(f, Tag::GPSLatitude) {
                    result.latval = Some(lat);
                } else if let Some(long) = is_lat_long(f, Tag::GPSLongitude) {
                    result.longval = Some(long);
                } else if let Some(s) = is_string(f, Tag::GPSLatitudeRef) {
                    result.latref = Some(s.to_string());
                } else if let Some(s) = is_string(f, Tag::GPSLongitudeRef) {
                    result.longref = Some(s.to_string());
                /*
                } else if let Some(s) = is_string(f, Tag::GPSImgDirectionRef) {
                    println!("  direction ref: {}", s);
                } else if let Some(s) = is_string(f, Tag::GPSImgDirection) {
                    println!("  direction: {}", s);
                */
                } else if let Some(d) = is_date(f, Tag::GPSDateStamp) {
                    result.gpsdate = Some(d);
                } else if let Some(hms) = is_time(f, Tag::GPSTimeStamp) {
                    result.gpstime = Some(hms);
                }
            }
            //println!("    {} ({}) {:?}", f.tag, f.thumbnail, f.value);
        }
        Ok(result)
    }

    pub fn date(&self) -> Option<NaiveDateTime> {
        // Note: I probably should return and store datetime with tz,
        // possibly utc, instead.
        // Also note: I used to prefer the gps date, as I belived that
        // to be more exact if present.  But at last one phone seems
        // to stick "the date of last time we had a gps position"
        // there rather than none if there is no "current" gps data.
        // So instead, use the gps date only as a fallback.
        if let Some(date) = self.dateval {
            Some(date)
        } else if let (&Some(date), &Some(time)) =
            (&self.gpsdate, &self.gpstime)
        {
            // The gps date and time should always be utc.
            // But time stored is in local time.
            // Note: Sometimes (when traveling) local time for the
            // picture may not be the same as local time for the
            // server.  That is not properly handled here.
            Some(Local.from_utc_datetime(&date.and_time(time)).naive_local())
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

fn is_lat_long(f: &Field, tag: Tag) -> Option<f64> {
    if f.tag == tag {
        match f.value {
            Value::Rational(ref v) if v.len() == 3 => {
                let d = 1. / 60.;
                Some(v[0].to_f64() + d * (v[1].to_f64() + d * v[2].to_f64()))
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

fn is_datetime(f: &Field, tag: Tag) -> Option<NaiveDateTime> {
    if f.tag == tag {
        single_ascii(&f.value)
            .and_then(|s| Ok(NaiveDateTime::parse_from_str(s, "%Y:%m:%d %T")?))
            .map_err(|e| {
                println!("ERROR: Expected datetime for {}: {:?}", tag, e);
            })
            .ok()
    } else {
        None
    }
}

fn is_date(f: &Field, tag: Tag) -> Option<NaiveDate> {
    if f.tag == tag {
        single_ascii(&f.value)
            .and_then(|s| Ok(NaiveDate::parse_from_str(s, "%Y:%m:%d")?))
            .map_err(|e| {
                println!("ERROR: Expected date for {}: {:?}", tag, e);
            })
            .ok()
    } else {
        None
    }
}

fn is_time(f: &Field, tag: Tag) -> Option<NaiveTime> {
    if f.tag == tag {
        match &f.value {
            // Some cameras (notably iPhone) uses fractional seconds.
            // Just round to whole seconds.
            &Value::Rational(ref v)
                if v.len() == 3 && v[0].denom == 1 && v[1].denom == 1 =>
            {
                NaiveTime::from_hms_opt(
                    v[0].num,
                    v[1].num,
                    v[2].num / v[2].denom,
                )
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

fn is_string(f: &Field, tag: Tag) -> Option<&str> {
    if f.tag == tag {
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

fn is_u32(f: &Field, tag: Tag) -> Option<u32> {
    if f.tag == tag {
        match &f.value {
            &Value::Long(ref v) if v.len() == 1 => Some(v[0]),
            &Value::Short(ref v) if v.len() == 1 => Some(u32::from(v[0])),
            v => {
                println!("ERROR: Unsuppored value for {}: {:?}", tag, v);
                None
            }
        }
    } else {
        None
    }
}

fn single_ascii(value: &Value) -> Result<&str, Error> {
    match value {
        &Value::Ascii(ref v) if v.len() == 1 => Ok(from_utf8(&v[0])?),
        &Value::Ascii(ref v) if v.len() > 1 => {
            for t in &v[1..] {
                if !t.is_empty() {
                    return Err(Error::Other(format!(
                        "Got {:?}, expected single ascii value",
                        v,
                    )));
                }
            }
            Ok(from_utf8(&v[0])?)
        }
        v => Err(Error::Other(format!(
            "Got {:?}, expected single ascii value",
            v,
        ))),
    }
}
