table! {
    attributions (id) {
        id -> Int4,
        name -> Varchar,
    }
}

table! {
    cameras (id) {
        id -> Int4,
        manufacturer -> Varchar,
        model -> Varchar,
    }
}

table! {
    people (id) {
        id -> Int4,
        slug -> Varchar,
        person_name -> Varchar,
    }
}

table! {
    photo_people (id) {
        id -> Int4,
        photo_id -> Int4,
        person_id -> Int4,
    }
}

table! {
    photo_places (id) {
        id -> Int4,
        photo_id -> Int4,
        place_id -> Int4,
    }
}

table! {
    photos (id) {
        id -> Int4,
        path -> Varchar,
        date -> Nullable<Timestamp>,
        grade -> Nullable<Int2>,
        rotation -> Int2,
        is_public -> Bool,
        camera_id -> Nullable<Int4>,
        attribution_id -> Nullable<Int4>,
        width -> Nullable<Int4>,
        height -> Nullable<Int4>,
    }
}

table! {
    photo_tags (id) {
        id -> Int4,
        photo_id -> Int4,
        tag_id -> Int4,
    }
}

table! {
    places (id) {
        id -> Int4,
        slug -> Varchar,
        place_name -> Varchar,
    }
}

table! {
    positions (id) {
        id -> Int4,
        photo_id -> Int4,
        latitude -> Int4,
        longitude -> Int4,
    }
}

table! {
    tags (id) {
        id -> Int4,
        slug -> Varchar,
        tag_name -> Varchar,
    }
}

table! {
    users (id) {
        id -> Int4,
        username -> Varchar,
        password -> Varchar,
    }
}

joinable!(photo_people -> people (person_id));
joinable!(photo_people -> photos (photo_id));
joinable!(photo_places -> photos (photo_id));
joinable!(photo_places -> places (place_id));
joinable!(photo_tags -> photos (photo_id));
joinable!(photo_tags -> tags (tag_id));
joinable!(photos -> attributions (attribution_id));
joinable!(photos -> cameras (camera_id));
joinable!(positions -> photos (photo_id));

allow_tables_to_appear_in_same_query!(
    attributions,
    cameras,
    people,
    photo_people,
    photo_places,
    photos,
    photo_tags,
    places,
    positions,
    tags,
    users,
);
