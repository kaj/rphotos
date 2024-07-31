// @generated automatically by Diesel CLI.

diesel::table! {
    attributions (id) {
        id -> Int4,
        name -> Varchar,
    }
}

diesel::table! {
    cameras (id) {
        id -> Int4,
        manufacturer -> Varchar,
        model -> Varchar,
    }
}

diesel::table! {
    people (id) {
        id -> Int4,
        slug -> Varchar,
        person_name -> Varchar,
    }
}

diesel::table! {
    photo_people (photo_id, person_id) {
        photo_id -> Int4,
        person_id -> Int4,
    }
}

diesel::table! {
    photo_places (photo_id, place_id) {
        photo_id -> Int4,
        place_id -> Int4,
    }
}

diesel::table! {
    photo_tags (photo_id, tag_id) {
        photo_id -> Int4,
        tag_id -> Int4,
    }
}

diesel::table! {
    photos (id) {
        id -> Int4,
        path -> Varchar,
        date -> Nullable<Timestamp>,
        grade -> Nullable<Int2>,
        rotation -> Int2,
        is_public -> Bool,
        camera_id -> Nullable<Int4>,
        attribution_id -> Nullable<Int4>,
        width -> Int4,
        height -> Int4,
    }
}

diesel::table! {
    places (id) {
        id -> Int4,
        slug -> Varchar,
        place_name -> Varchar,
        osm_id -> Nullable<Int8>,
        osm_level -> Nullable<Int2>,
    }
}

diesel::table! {
    positions (id) {
        id -> Int4,
        photo_id -> Int4,
        latitude -> Int4,
        longitude -> Int4,
    }
}

diesel::table! {
    tags (id) {
        id -> Int4,
        slug -> Varchar,
        tag_name -> Varchar,
    }
}

diesel::table! {
    users (id) {
        id -> Int4,
        username -> Varchar,
        password -> Varchar,
    }
}

diesel::joinable!(photo_people -> people (person_id));
diesel::joinable!(photo_people -> photos (photo_id));
diesel::joinable!(photo_places -> photos (photo_id));
diesel::joinable!(photo_places -> places (place_id));
diesel::joinable!(photo_tags -> photos (photo_id));
diesel::joinable!(photo_tags -> tags (tag_id));
diesel::joinable!(photos -> attributions (attribution_id));
diesel::joinable!(photos -> cameras (camera_id));
diesel::joinable!(positions -> photos (photo_id));

diesel::allow_tables_to_appear_in_same_query!(
    attributions,
    cameras,
    people,
    photo_people,
    photo_places,
    photo_tags,
    photos,
    places,
    positions,
    tags,
    users,
);
