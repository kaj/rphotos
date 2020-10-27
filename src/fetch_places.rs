use crate::dbopt::PgPool;
use crate::models::{Coord, Place};
use crate::DbOpt;
use diesel::prelude::*;
use diesel::result::{DatabaseErrorKind, Error as DieselError};
use log::{debug, info};
use reqwest::{self, Client, Response};
use serde_json::Value;
use slug::slugify;
use structopt::StructOpt;

#[derive(StructOpt)]
#[structopt(rename_all = "kebab-case")]
pub struct Fetchplaces {
    #[structopt(flatten)]
    db: DbOpt,
    #[structopt(flatten)]
    overpass: OverpassOpt,

    /// Max number of photos to use for --auto
    #[structopt(long, short, default_value = "5")]
    limit: i64,
    /// Fetch data for photos with position but lacking places.
    #[structopt(long, short)]
    auto: bool,
    /// Image ids to fetch place data for
    photos: Vec<i32>,
}

impl Fetchplaces {
    pub async fn run(&self) -> Result<(), super::adm::result::Error> {
        let db = self.db.create_pool()?;
        if self.auto {
            println!("Should find {} photos to fetch places for", self.limit);
            use crate::schema::photo_places::dsl as place;
            use crate::schema::positions::dsl as pos;
            let result = pos::positions
                .select((pos::photo_id, (pos::latitude, pos::longitude)))
                .filter(pos::photo_id.ne_all(
                    place::photo_places.select(place::photo_id).distinct(),
                ))
                .order(pos::photo_id.desc())
                .limit(self.limit)
                .load::<(i32, Coord)>(&db.get()?)?;
            for (photo_id, coord) in result {
                println!("Find places for #{}, {:?}", photo_id, coord);
                self.overpass.update_image_places(&db, photo_id).await?;
            }
        } else {
            for photo in &self.photos {
                self.overpass.update_image_places(&db, *photo).await?;
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, StructOpt)]
#[structopt(rename_all = "kebab-case")]
pub struct OverpassOpt {
    /// How to connect to the overpass API.
    ///
    /// See https://wiki.openstreetmap.org/wiki/Overpass_API for
    /// available servers and policies.
    #[structopt(long, env = "OVERPASS_URL")]
    overpass_url: String,
}

impl OverpassOpt {
    pub async fn update_image_places(
        &self,
        db: &PgPool,
        image: i32,
    ) -> Result<(), Error> {
        use crate::schema::positions::dsl::*;
        let coord = positions
            .filter(photo_id.eq(image))
            .select((latitude, longitude))
            .first::<Coord>(
                &db.get().map_err(|e| Error::Pool(image, e.to_string()))?,
            )
            .optional()
            .map_err(|e| Error::Db(image, e))?
            .ok_or_else(|| Error::NoPosition(image))?;
        debug!("Should get places for #{} at {:?}", image, coord);
        let data = Client::new()
            .post(&self.overpass_url)
            .body(format!("[out:json];is_in({},{});out;", coord.x, coord.y))
            .send()
            .await
            .and_then(Response::error_for_status)
            .map_err(|e| Error::Server(image, e))?
            .json::<Value>()
            .await
            .map_err(|e| Error::Server(image, e))?;

        if let Some(elements) = data
            .as_object()
            .and_then(|o| o.get("elements"))
            .and_then(Value::as_array)
        {
            let c = db.get().map_err(|e| Error::Pool(image, e.to_string()))?;
            for obj in elements {
                if let (Some(t_osm_id), Some((name, level))) =
                    (osm_id(obj), name_and_level(obj))
                {
                    debug!("{}: {} (level {})", t_osm_id, name, level);
                    let place = get_or_create_place(&c, t_osm_id, name, level)
                        .map_err(|e| Error::Db(image, e))?;
                    if place.osm_id.is_none() {
                        debug!("Matched {:?} by name, update osm info", place);
                        use crate::schema::places::dsl::*;
                        diesel::update(places)
                            .filter(id.eq(place.id))
                            .set((
                                osm_id.eq(Some(t_osm_id)),
                                osm_level.eq(level),
                            ))
                            .execute(&c)
                            .map_err(|e| Error::Db(image, e))?;
                    }
                    use crate::models::PhotoPlace;
                    use crate::schema::photo_places::dsl::*;
                    let q = photo_places
                        .filter(photo_id.eq(image))
                        .filter(place_id.eq(place.id));
                    if q.first::<PhotoPlace>(&c).is_ok() {
                        debug!(
                            "Photo #{} already has {} ({})",
                            image, place.id, place.place_name
                        );
                    } else {
                        diesel::insert_into(photo_places)
                            .values((
                                photo_id.eq(image),
                                place_id.eq(place.id),
                            ))
                            .execute(&c)
                            .map_err(|e| Error::Db(image, e))?;
                    }
                } else {
                    info!("Unused area: {}", obj);
                }
            }
        }
        Ok(())
    }
}

fn osm_id(obj: &Value) -> Option<i64> {
    obj.get("id").and_then(Value::as_i64)
}

#[rustfmt::skip] // This data is written in a more compact style
static KNOWN: [(&str, &[(&str, i16)]); 15] = [
    ("leisure", &[
        ("garden", 18),
        ("nature_reserve", 12),
        ("park", 14),
        ("pitch", 15),
        ("playground", 16),
    ]),
    ("tourism", &[
        ("attraction", 16),
        ("theme_park", 14),
        ("zoo", 14),
    ]),
    ("boundary", &[
        ("national_park", 14),
        ("historic", 7), // Seems to be mainly "Landskap"
    ]),
    ("landuse", &[
        ("allotments", 14),
        ("commercial", 12),
        ("grass", 13),
        ("industrial", 11),
        ("meadow", 16),
        ("railway", 13),
        ("residential", 11),
        ("retail", 13),
    ]),
    ("highway", &[
        ("pedestrian", 15),  // torg
        ("residential", 15), // torg?
        ("rest_area", 16), // rastplats
        ("services", 16), // rastplats / vägkrog
    ]),
    ("public_transport", &[
        ("station", 18),
    ]),
    ("amenity", &[
        ("bus_station", 16),
        ("exhibition_center", 20),
        ("kindergarten", 15),
        ("place_of_worship", 15),
        ("school", 14),
        ("university", 12),
    ]),
    ("aeroway", &[
        ("aerodrome", 14),
    ]),
    ("water", &[
        ("lake", 15),
    ]),
    ("waterway", &[
        ("riverbank", 16),
    ]),
    ("man_made", &[
        ("bridge", 17),
    ]),
    ("place", &[
        ("city_block", 17),
        ("island", 13),
        ("islet", 17),
        ("penisula", 13),
        ("region", 8),
        ("square", 18),
        ("suburb", 11),
    ]),
    ("natural", &[
        ("bay", 14),
        ("beach", 15),
        ("scrub", 18),
        ("wood", 14),
    ]),
    ("building", &[
        ("exhibition_center", 19),
        ("sports_hall", 19),
        ("", 20), // MAGIC: Empty value means default
    ]),
    ("political_division", &[
        ("canton", 9),
    ]),
];

fn name_and_level(obj: &Value) -> Option<(&str, i16)> {
    let tags = obj.get("tags")?;
    let name = tags
        .get("name:sv")
        // .or_else(|| tags.get("name:en"))
        .or_else(|| tags.get("name"))
        .and_then(Value::as_str)?;
    let level = tags
        .get("admin_level")
        .and_then(Value::as_str)
        .and_then(|l| l.parse().ok())
        .or_else(|| {
            KNOWN
                .iter()
                .find_map(|(name, values)| tag_level(tags, name, values))
        })?;

    debug!("{} is level {}", name, level);
    Some((name, level))
}

fn tag_level(tags: &Value, name: &str, values: &[(&str, i16)]) -> Option<i16> {
    let value = tag_str(tags, name)?;
    for (vs, vi) in values {
        if &value == vs || vs.is_empty() {
            return Some(*vi);
        }
    }
    None
}

fn tag_str<'a>(tags: &'a Value, name: &str) -> Option<&'a str> {
    tags.get(name).and_then(Value::as_str)
}

fn get_or_create_place(
    c: &PgConnection,
    t_osm_id: i64,
    name: &str,
    level: i16,
) -> Result<Place, diesel::result::Error> {
    use crate::schema::places::dsl::*;
    places
        .filter(
            osm_id
                .eq(Some(t_osm_id))
                .or(place_name.eq(name).and(osm_id.is_null())),
        )
        .first::<Place>(c)
        .or_else(|_| {
            let mut result = diesel::insert_into(places)
                .values((
                    place_name.eq(&name),
                    slug.eq(&slugify(&name)),
                    osm_id.eq(Some(t_osm_id)),
                    osm_level.eq(Some(level)),
                ))
                .get_result::<Place>(c);
            let mut attempt = 1;
            while is_duplicate(&result) && attempt < 25 {
                info!("Attempt #{} got {:?}, trying again", attempt, result);
                attempt += 1;
                let name = format!("{} ({})", name, attempt);
                result = diesel::insert_into(places)
                    .values((
                        place_name.eq(&name),
                        slug.eq(&slugify(&name)),
                        osm_id.eq(Some(t_osm_id)),
                        osm_level.eq(Some(level)),
                    ))
                    .get_result::<Place>(c);
            }
            result
        })
}

fn is_duplicate<T>(r: &Result<T, DieselError>) -> bool {
    matches!(
        r,
        Err(DieselError::DatabaseError(DatabaseErrorKind::UniqueViolation, _))
    )
}

#[derive(Debug)]
pub enum Error {
    NoPosition(i32),
    Db(i32, diesel::result::Error),
    Pool(i32, String),
    Server(i32, reqwest::Error),
}

#[cfg(test)]
mod test {
    use super::name_and_level;
    use serde_json::Value;

    #[test]
    fn test_long_reply() -> Result<(), Box<dyn std::error::Error>> {
        let data: Value = TEST_DATA.parse()?;
        let elements = data
            .as_object()
            .and_then(|o| o.get("elements"))
            .and_then(Value::as_array)
            .unwrap();

        assert_eq!(
            elements.iter().map(name_and_level).collect::<Vec<_>>(),
            [
                Some(("Stamparken", 14)),
                Some(("Älvsjö postort", 10)),
                Some(("Älvsjö postort", 10)),
                Some(("Sverige", 2)),
                Some(("Svealand", 7)),
                Some(("Stockholms län", 4)),
                Some(("Stockholms kommun", 7)),
                Some(("Landskapet Södermanland", 7)),
                Some(("Sveriges Landskap", 5)),
                Some(("Enskede-Årsta-Vantörs stadsdelsområde", 9)),
                Some(("Södertörn", 13)),
                Some(("Älvsjö postort", 10))
            ],
        );
        Ok(())
    }

    #[test]
    fn specific_building() -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(
            name_and_level(
                &r##"{"id":2442330563,"tags":{"building":"exhibition_center","name":"Älvsjömässan"},"type":"area"}"##
                    .parse()?
            ),
            Some(("Älvsjömässan", 19))
        );
        Ok(())
    }
    #[test]
    fn default_building() -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(
            name_and_level(
                &r##"{"id":2442330563,"tags":{"building":"anything","name":"Home"},"type":"area"}"##
                    .parse()?
            ),
            Some(("Home", 20))
        );
        Ok(())
    }
    #[test]
    fn nodefault_leisure() -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(
            name_and_level(
                &r##"{"id":2442330563,"tags":{"leisure":"some","name":"Home"},"type":"area"}"##
                    .parse()?
            ),
            None
        );
        Ok(())
    }

    static TEST_DATA: &str = r##"{"elements":[{"id":2442330563,"tags":{"leisure":"park","name":"Stamparken"},"type":"area"},{"id":2619894464,"tags":{"admin_level":"10","boundary":"administrative","history":"Retrieved from v4","name":"Älvsjö postort"},"type":"area"},{"id":2701625178,"tags":{"admin_level":"10","boundary":"administrative","history":"Retrieved from v3","name":"Älvsjö postort"},"type":"area"},{"id":3600052822,"tags":{"ISO3166-1":"SE","ISO3166-1:alpha2":"SE","ISO3166-1:alpha3":"SWE","ISO3166-1:numeric":"752","admin_level":"2","alt_name:eo":"Svedujo","boundary":"administrative","flag":"http://upload.wikimedia.org/wikipedia/commons/4/4c/Flag_of_Sweden.svg","int_name":"Sweden","is_in:continent":"Europe","name":"Sverige","name:ab":"Швециа","name:ace":"Swèdia","name:af":"Swede","name:ak":"Sweden","name:als":"Schweden","name:am":"ስዊድን","name:an":"Suecia","name:ang":"Swēoland","name:ar":"السويد","name:arc":"ܣܘܝܕ","name:arz":"السويد","name:ast":"Suecia","name:az":"İsveç","name:ba":"Швеция","name:bar":"Schwedn","name:bat-smg":"Švedėjė","name:bcl":"Suesya","name:be":"Швецыя","name:be-tarask":"Швэцыя","name:bg":"Швеция","name:bi":"Sweden","name:bm":"Swedi","name:bn":"সুইডেন","name:bo":"སི་ཝེ་དེན།","name:bpy":"সুইডেন","name:br":"Sveden","name:bs":"Švedska","name:bxr":"Швед","name:ca":"Suècia","name:cdo":"Sôi-diēng","name:ce":"Швеци","name:ceb":"Suwesya","name:chr":"ᏍᏫᏕᏂ","name:ckb":"سوید","name:co":"Svezia","name:crh":"İsveç","name:cs":"Švédsko","name:csb":"Szwedzkô","name:cu":"Свєньско","name:cv":"Швеци","name:cy":"Sweden","name:da":"Sverige","name:de":"Schweden","name:diq":"İswec","name:dsb":"Šwedska","name:dv":"ސުވިޑަން","name:dz":"སུའི་ཌན་","name:ee":"Sweden","name:el":"Σουηδία","name:en":"Sweden","name:eo":"Svedio","name:es":"Suecia","name:et":"Rootsi","name:eu":"Suedia","name:ext":"Suécia","name:fa":"سوئد","name:fi":"Ruotsi","name:fiu-vro":"Roodsi","name:fo":"Svøríki","name:fr":"Suède","name:frp":"Suède","name:frr":"Swärik","name:fur":"Svezie","name:fy":"Sweden","name:ga":"An tSualainn","name:gag":"Şvețiya","name:gan":"瑞典","name:gd":"An t-Suain","name:gl":"Suecia","name:gn":"Suesia","name:gu":"સ્વિડન","name:gv":"Yn Toolynn","name:hak":"Shuì-tién","name:haw":"Kuekene","name:he":"שבדיה","name:hi":"स्वीडन","name:hif":"Sweden","name:hr":"Švedska","name:hsb":"Šwedska","name:ht":"Syèd","name:hu":"Svédország","name:hy":"Շվեդիա","name:ia":"Svedia","name:id":"Swedia","name:ie":"Svedia","name:ilo":"Suésia","name:io":"Suedia","name:is":"Svíþjóð","name:it":"Svezia","name:iu":"ᔅᕗᕆᑭ","name:ja":"スウェーデン","name:jbo":"sueriges","name:jv":"Swédia","name:ka":"შვედეთი","name:kaa":"Shvetsiya","name:kab":"Sswid","name:kbd":"Шуэц","name:kg":"Suedi","name:ki":"Sweden","name:kk":"Швеция","name:kl":"Svenskit Nunaat","name:km":"ស៊ុយអែត","name:kn":"ಸ್ವೀಡನ್","name:ko":"스웨덴","name:koi":"Шведму","name:krc":"Швеция","name:ku":"Swêd","name:kv":"Швеция","name:kw":"Swedherwyk","name:ky":"Швеция","name:la":"Suecia","name:lad":"Suesia","name:lb":"Schweden","name:lez":"Швеция","name:lg":"Swiiden","name:li":"Zwede","name:lij":"Sveçia","name:lmo":"Svezia","name:ln":"Swédi","name:lt":"Švedija","name:ltg":"Švedeja","name:lv":"Zviedrija","name:lzh":"瑞典","name:mdf":"Шведмастор","name:mg":"Soeda","name:mhr":"Швеций","name:mi":"Huītene","name:mk":"Шведска","name:ml":"സ്വീഡൻ","name:mn":"Швед","name:mr":"स्वीडन","name:ms":"Sweden","name:mt":"Żvezja","name:my":"ဆွီဒင်နိုင်ငံ","name:myv":"Швеция Мастор","name:mzn":"سوئد","name:na":"Widen","name:nah":"Suecia","name:nan":"Sūi-tián","name:nap":"Sguezia","name:nds":"Sweden","name:nds-nl":"Sveden","name:ne":"स्वीडेन","name:new":"स्विदेन","name:nl":"Zweden","name:nn":"Sverige","name:no":"Sverige","name:nov":"Suedia","name:nrm":"Suède","name:oc":"Suècia","name:or":"ସ୍ଵିଡେନ","name:os":"Швеци","name:pa":"ਸਵੀਡਨ","name:pag":"Suesia","name:pam":"Sweden","name:pap":"Suecia","name:pcd":"Suède","name:pih":"Swiiden","name:pl":"Szwecja","name:pms":"Svessia","name:pnb":"سویڈن","name:pnt":"Σουηδία","name:ps":"سويډن","name:pt":"Suécia","name:qu":"Suwidsuyu","name:rm":"Svezia","name:ro":"Suedia","name:roa-rup":"Suidia","name:roa-tara":"Svezzie","name:ru":"Швеция","name:rue":"Швеція","name:rw":"Suwede","name:sa":"स्वीडन","name:sah":"Швеция","name:sc":"Isvetzia","name:scn":"Svezzia","name:sco":"Swaden","name:se":"Ruoŧŧa","name:sh":"Švedska","name:sje":"Sverrje","name:sk":"Švédsko","name:sl":"Švedska","name:sm":"Sweden","name:smn":"Ruotâ","name:sms":"Ruõcc","name:so":"Iswiidhan","name:sq":"Suedia","name:sr":"Шведска","name:ss":"ISwideni","name:stq":"Sweeden","name:su":"Swédia","name:sv":"Sverige","name:sw":"Uswidi","name:szl":"Szwecyjo","name:ta":"சுவீடன்","name:te":"స్వీడన్","name:tet":"Suésia","name:tg":"Шветсия","name:th":"ประเทศสวีเดน","name:tk":"Şwesiýa","name:tl":"Suwesya","name:tok":"ma Wensa","name:tpi":"Suwidan","name:tr":"İsveç","name:tt":"Швеция","name:tw":"Sweden","name:tzl":"Sveiria","name:udm":"Швеция","name:ug":"شۋېتسىيە","name:uk":"Швеція","name:ur":"سویڈن","name:uz":"Shvetsiya","name:vec":"Svèsia","name:vep":"Ročinma","name:vi":"Thụy Điển","name:vls":"Zweedn","name:vo":"Svedän","name:wa":"Suwedwesse","name:war":"Suwesya","name:wo":"Suweed","name:wuu":"瑞典","name:xal":"Сведин Нутг","name:xmf":"შვედეთი","name:yi":"שוועדן","name:yo":"Swídìn","name:yue":"瑞典","name:zea":"Zweden","name:zh":"瑞典","name:zh-Hans":"瑞典","name:zu":"ISwidi","official_name":"Konungariket Sverige","official_name:cs":"Švédské království","official_name:el":"Βασίλειο της Σουηδίας","official_name:eo":"Reĝlando Svedio","timezone":"Europe/Stockholm","type":"boundary","wikidata":"Q34","wikipedia":"sv:Sverige"},"type":"area"},{"id":3600054219,"tags":{"boundary":"historic","cg_ref":"SE-Svealand","name":"Svealand","name:de":"Landesteil Svealand","name:en":"Region Svealand","name:fi":"Sveanmaa","name:ru":"Свеаланд","type":"boundary","wikidata":"Q203835","wikipedia":"sv:Svealand"},"type":"area"},{"id":3600054391,"tags":{"ISO3166-2":"SE-AB","admin_level":"4","boundary":"administrative","name":"Stockholms län","name:ar":"محافظة ستوكهولم","name:de":"Provinz Stockholm","name:en":"Stockholm County","name:es":"Provincia de Estocolmo","name:fi":"Tukholman lääni","name:fr":"Comté de Stockholm","name:ru":"Стокгольм","ref":"AB","ref:fips":"SW26","ref:nuts:1":"SE1","ref:nuts:2":"SE11","ref:nuts:3":"SE110","ref:se:scb":"01","type":"boundary","wikidata":"Q104231","wikipedia":"sv:Stockholms län"},"type":"area"},{"id":3600398021,"tags":{"KNKOD":"0180","admin_level":"7","boundary":"administrative","name":"Stockholms kommun","name:eo":"Stokholmo","name:es":"Estocolmo","name:fi":"Tukholman kunta","name:ru":"Стокгольм","name:sv":"Stockholms kommun","official_name":"Stockholms kommun","ref":"0180","ref:scb":"0180","short_name":"Stockholm","type":"boundary","wikidata":"Q506250","wikipedia":"sv:Stockholms kommun"},"type":"area"},{"id":3603985130,"tags":{"alt_name":"Sörmland","boundary":"historic","name":"Landskapet Södermanland","name:de":"Landschaft Södermanland","name:en":"Province of Södermanland","name:ru":"Сёдерманланд","short_name":"Södermanland","type":"boundary","wikidata":"Q626062","wikipedia":"sv:Södermanland"},"type":"area"},{"id":3604222805,"tags":{"admin_level":"5","boundary":"administrative","name":"Sveriges Landskap","type":"boundary","wikidata":"Q193556"},"type":"area"},{"id":3605695996,"tags":{"admin_level":"9","boundary":"administrative","name":"Enskede-Årsta-Vantörs stadsdelsområde","short_name":"Enskede-Årsta-Vantör","type":"boundary","wikidata":"Q606458","wikipedia":"sv:Enskede-Årsta-Vantörs stadsdelsområde"},"type":"area"},{"id":3605813353,"tags":{"name":"Södertörn","official_name":"Södertörn-Nacka","place":"island","type":"multipolygon","wikidata":"Q2031843"},"type":"area"},{"id":3608844985,"tags":{"admin_level":"10","boundary":"administrative","history":"Retrieved from v4","name":"Älvsjö postort","type":"boundary"},"type":"area"}],"generator":"Overpass API 0.7.56.6 474850e8","osm3s":{"copyright":"The data included in this document is from www.openstreetmap.org. The data is made available under ODbL.","timestamp_areas_base":"2020-09-01T21:05:02Z","timestamp_osm_base":"2020-09-01T21:55:03Z"},"version":0.6}"##;
}
