# Changelog for rphotos

All notable changes to this project will be documented in this file.
The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

## Unreleased

* Database access is now async (PR #10).
  - Use `diesel-async` with deadpool feature for database access.
  - A bunch of previously synchronous handlers are now async.
  - Some `.map` and simliar replaced with `if` blocks or `for` loops.
* Refactored query parsing and facet handling in search (PR #11).
  Should be more efficient now, especially for negative facets.
* Improved diagnosics and date format flexibility in parsing exif data.
* Avoid an extra query for the positions in search.
* Links by time is 404 if no images found in query.
* Some random cleanup and refactoring.


## Release 0.11.10 (2023-02-11)

* Bugfix in month start calculation.


## Release 0.11.8 (2023-01-29)

* Check for null bytes in autocomplete patterns, make them result in a
  400 bad request rather than a 500 internal server error.
* Changed zoom-on-click to use fullscreen.  I think that is an
  improvement especially on mobile.
* Improved logging by using tracing and tracing-subscriber rather than
  log and env_logger.
* Four more kinds of OSM areas to recognize.
* Added a favicon.
* Update diesel to 2.0.0: Mainly most operations now needs a `&mut
  PgConnection`.  Also, getting parts of dates are now done by sql
  functions.
* Update ructe to 0.16.0.
* Update clap to 4.0.18.
* Add this changelog.

## Release 0.11.6 (2022-07-10)

* Another kind of OSM area to recognize.
* Update from structopt to clap 3.2.8.
* Tell robots not to index the login form.

## Release 0.11.4 (2022-02-20)
    
Be less noicy about exif data when finding images.

## Release 0.11.2 (2022-02-20)
    
Improve reading the date of images.  Prefer camera times over gps
times, since sometimes and old position is used, and than that old
time is saved in gps time field.

## Release 0.11.0 (2022-02-20)
    
* Much improved error handling.
* Improved warp routing.
* Minor improvements in site css and amdin js.
* Some more known places.
* Update ructe to 0.14.0.
* Update env_logger to 0.9.0.
* Update image to 0.24.0.
* Use lazy-regex 2.2.2 for cleaner and more efficient regex checking.
* Update to rust edition 2021.


## Release 0.10.0 (2021-07-04)

* Specify MIT license (Issue #6).
* Better tag and search suggestions.
* Minor markup/style improvement.
* Recognize more osm places.
* Json body for all api errors.
* Proper logging for errors.
* Update to tokio 1.0 (reqwest 0.11.0, tokio 1.0.2, warp 0.3.0)
* Update regex.
* Update rand to 0.8.
* Move from travis to github actions.
* Update r2d2-memcache to 0.6.
* Update ructe to 0.13.
* Some code cleanup.


## Release 0.9.0 (2020-10-28)

* Improve performance by loading jpegs "pre-scaled" (PR #5).
* Add number of hits to search results.
* More kinds of OSM places to recognize.
* Update env_logger to 0.8.1.
* Use `matches!` and some other code cleanup and refactoring.


## Release 0.8.10 (2020-10-09)

* Refactor search time interval.
* Add header to multiyear groups (E.g. "2018 - 20" or "1989 - 2003").

## Release 0.8.8 (2020-09-02)

* Fix map for anonymous viewers.
* Test and refactor `fetch_places::name_and_level`
* Add one more place.

## Release 0.8.6 (2020-08-15)

* Redesign details view.

  The details view is now implemented with css grid, in a full-window
  no-scroll layout (except for small screens, where the design is still
  sequential and scroll is used)
    
  This incudes some cleanup of the markup and corresponding changes in the
  admin script.
    
* Update ructe to 0.12.0.
* Larger small and medium images.
* Slightly more pics per page in split lists.
* Use more OSM places.
* Update README.

## Release 0.8.4 (2020-07-07)
    
* Improve async image scalig by using tokio spawn_blocking.
* Get size from actual image if it is missing in exif data.
* Add "close" link for the keyboard help.
* Improved db pool initialization.
* Some cleanup / refactoring.

## Release 0.8.0 (2020-05-08)
    
* Add og metadata to image indexes.
* Update warp, use async/await.
* Lots of code cleanup and dependency clarification.
* Update ructe to 0.11.4.
* Update kamadak-exif to 0.5.x.
* Update image to 0.23.x.
* Update r2d2-memcached to 0.5.0.
* Disable legacy password hashers (all but pbkdf2 for now).


## Release 0.7.0 (2019-11-02)

* Add API endpoints for getting some image metadata, making images public,
  and logging in (to make images public or get non-public metadata).
* Allow use of authorization header.
* Disable default unused diesel features (mainly disable support for tables
  with more than 16 columns, which should improve compile times
  noticeably).
* Disable unused default features of warp.
* Update `reqwest`.
* Update `dotenv` to 0.15.
* Another kind of place to recognize.


## Release 0.6.12 (2019-10-08)

* Fix search zoomig again.

## Release 0.6.10 (2019-10-08)

* Use more places from OSM.
* Calc function is fixed in rsass 0.11.2, remove workarounds.
* Refactor facet search.
* Some cleanup.
* Fix toggling for `pos` and `!pos` search.

## Release 0.6.8 (2019-09-21)

* Implement negative searching: Seach for images with or _without_
  specific tags, places or people.
* Rewrote GlobalContext.verify_key to get rid of some clippy warnings.
* Fix some warnings with rustc 1.37.
* Update structopt to 0.3, r2d2-memcache to 0.4.0, and ructe to 0.7.2.
* Use `medallion` 2.3.1 instead of `jwt`.

## Release 0.6.6 (2019-08-03)
    
* Keep `pos` / `!pos` when "zooming" in search.

## Release 0.6.4 (2019-08-01)

* Magic `pos` / `!pos` search.
* Make overpass API url configurable.
* Shorter db usage from pool.
  Don't keep a db handle checked out from the pool for the entire
  request, but instead check out one when needed.  Hopefully, this means
  it won't keep a db connection busy while scaling an image.

## Release 0.6.2 (2019-07-23)

* Bugfix in search results; when klicking a group, keep the search query.

## Release 0.6.0 (2019-07-23)

* Implement search.
* Update ructe to 0.7.0 and image to 0.22.
* Accept more OSM place types.


## Release 0.5.14 (2019-07-05)

* Lock reqwest version at 0.9.17 to avoid async problem.
* Use more OSM places.
* Update rand to 0.7.0.
* Refactor:  Move some code

## Release 0.5.12 (2019-06-30)

* Fix kebab-case in precache args.
* Remove the find-sizes subcommand (redundant since 0.5.10).

## Release 0.5.10 (2019-06-06)

* Make photo size required in db.
* Improve size and position of admin buttons.
* Fix a commandline bug from structopt transition.
* Add a command to make images public by tag.
* Update brotli to native rust implementation.
* Some internal code cleanup.

## Release 0.5.8 (2019-05-18)

* Use structopt command line handling (PR #4).
* Don't exclude raw images from findsizes.
* Fix path for fallback image reading in find-sizes.

## Release 0.5.6 (2019-05-15)
    
* Fallback to reading actual image data if exif size fails in find-sizes.

## Release 0.5.4 (2019-05-12)

* Improve error reporting in ExifData::read_from.

## Release 0.5.2 (2019-05-12)

* Add a command to fix missing image sizes in db.
* More work on sizing in image lists.
* Update dotenv to 0.14.0.
* Find images that has position but no places in a more efficient way in
  fetch-places.

## Release 0.5.0 (2019-05-05)

* Use warp instead of nickel (PR #2).
* Use edition 2018 (PR #3).
* Use pooled r2d2-memcache instead of the old memcached-rust.
* Updated style: Use more screen for images and less for borders and
  frames.
* Fixed broken keyboard stuff in admin.
* Updated leaflet.
* Improved the split function, this way it should never create zero-size
  sublists.
* Update some dependencies.
* Some random cleanup.
* Use stable rustfmt.


## Release 0.4.26 (2018-11-03)

* Add opengraph metadata to photo details.
* Add positions map to year view.
* Multiple improvements in fetch_places.
  - Allow different places with the same name.
  - Improved logging and error handling.
* Use serde (rather than rustc-serialize) for json responses and fetch_places.
* Update ructe to 0.5, reqwest to 0.9, and image to 0.20.

## Release 0.4.24 (2018-09-08)

* Enable submitting location by pressing enter.
* Allow deeper zoom when placing image.
* Add time limit to `precache` command.
* Added auto arg to `fetchplaces` command.

## Release 0.4.22 (2018-09-07)

* Get location names from OSM: (PR #1)
  - Improve presentation of places on a photo.
  - Put places after tags on details.
  - Fetch OSM places when setting image position.
  - Added a command line subcommand to fetch places for specified images.
* Quit using `diesel_infer_schema`.  Instead, let diesel keep `schema.rs`
  up-to-date when migrating.
* Test and improve splitting a group.
* Upgrade dotenv and diesel dependencies.
  - Use r2d2 as diesel feature, not separate crate.

## Release 0.4.20 (2018-08-12)

* Add positions to month and on-this-day views.
* Nicer positioning by local storage (start at last set position).
* Smaller clusters (35, rather than default 80) gives better map views.
* Minor exif handling improvement.
* Improve diesel usage (avoid full-query verbatim sql).
* Update diesel to 1.2.x.
* Allow (keep) manually corrected positions when finding photos.

## Release 0.4.18 (2018-07-19)

* Add leaflet clustering; Nice handling of maps with extreme numbers of photos.
* Add photo previews to map markers.
* Update rand and memcached-rs.
* Some code-style cleanup.

## Release 0.4.16 (2018-07-15)
    
* Add admin ux to set image position.
* Fix leaflet markers. Sometimes the markers would not view, that was
  because the trick to find the correct url for the marker images didn't
  work when the css might load after the js.  So just load the css before
  the js instead.
* Update ructe to 0.4.x.
* Some cleanups, mainly clippy-suggested.

## Release 0.4.14 (2018-04-28)
    
* Better / faster image scaling from updated image dependency.

## Release 0.4.12 (2018-04-08)
    
* Fix error that broke the front page.

## Release 0.4.10 (2018-04-08)
    
* Store width and height of images.  Use it to calculate width and
  height of small images in lists.
    
  Since the old database should be migratable to the new, the width
  and height fields are nullable.  I aim to make them not nullable
  later, after all images in the database has got width and height.

* Don't override picture orientation for existing images in findphotos.
* Don't let the map hide dropdowns.
* Shorter about string in footer.
* Use kamadak-exif instead of rexif.
* Updates Diesel to 1.1.0, get rid of unnecessary New* structs.
* Miscellaneous code cleanup.

## Release 0.4.8 (2018-02-11)

* Some map-relate improvements:
  - Use a local copy of leaflet, and update to 1.3.1.
  - Disable `scrollwWheelZoom` since it interferes with page scrolling.
  - Add maps to image lists.
  - DRYer map initialization.
  - Limit height of map to 60% of browser height.
  - Improve map scaling and rescaling.
* Log if ther is a problem loading positions.
* Improve next/prev for images with the same time stamp.
* Put admin forms before map.
* Store compressed assets only if worth it (for some asset formats,
  applying gzip or brotli compression will actually make it larger or only
  marginally smaller.  In those cases, don't write the compressed file ad
  all when running `storestatics`).
* Dont split static path names (a static filed might be called
  somename.suffix, but there is no need to assume that, and absolutley no
  need to copy the strings separatley and format them together again when
  we can just borrow the relevant substring directly from the request).
* Some cleanup and rustfmt.

## Release 0.4.6 (2018-02-05)

* Move js for map from details.html to `ux.js`.
* Support Exif orientation 0 (disabled?).
* Use nightly distributed rustfmt-preview.
* Use diesel 1.0.0 (and fix deprecated diesel type usage).
* Update memcached and env_logger dependencies.
* Drop support for rustc 1.19 (and allow clap to update).
* Update image to 0.18.
* Rustfmt.

## Release 0.4.4 (2017-12-30)

* Improve time-cluster grouping: Make groups no shorter than 8 and no
  longer than 16 photos.  Also, make time and not only number of images a
  factor when deciding which group to split next.
* Improve showing of time spans.
* Allow `"_"` in `next`.
* Use the slug crate for generating slugs.
* Lock r2d2 at version used by r2d2-diesel.
* Limit clap version to avoid problem with bitflags on rustc 1.19.
* Specify rand version.
* Use flate2 version 1.x, there was a slight API change.
* Move some javascript from template to `ux.js`.
* Shorten and simplify `admin.js`.
* Improve layout (I hope)
* Some refactoring / cleanup.
* `async` AND `defer` is apparently a bad idea.
* Be sure to write all of the compressed file.

## Release 0.4.2 (2017-11-25)

* Improve tag completion: Less is more (max 10 answers, not 15), and
  BadRequest rather than 404 is q query parameter is missing.
* Add admin UX to grade images.
* Rustfmt update.
* Some cleanup and fixes in code, styling and markup.

## Release 0.4.0 (2017-11-20)

* Add time-clustered grouping for day, person, tag, and place views.
* Implement next and prev links.
* More access keys, including a helpful listing.
* Some code cleanup.


## Release 0.3.6 (2017-11-11)

* Add UX to note people in pictures.
* Update and reenable rustfmt, update code to the new recommended style.
* The `/random` url now uses a redirect rather than just rendering a random
  photo page.  Also, a link to `/random` is added to the menu.
* Remove `readkpa` subcommand, that was never intended for general
  consumption anyway, it was hardcoded to match configurations in my
  kphotoalbum setup.
* Some minor cleanup.

## Release 0.3.4 (2017-11-06)

* Show date and time below all images (if known).
* Improve UX for tag form.

## Release 0.3.2 (2017-11-05)

* Nicer implementation of `find_image_data`.
* Add UI to add tags to images.
* Silly feature; an url for a random image.
* Update ructe.
* Lock image and xml-rs versions (to Avoid updating bitflags dependency to
  1.0.0, which does not support rustc 1.19.0 (which is what I currently
  have on my FreeBSD server)).
* Drop support for rust 1.18.

## Release 0.3.0 (2017-09-27)

* Merge server and adm to single binary.
* Fix memcache problem: Thirty days is max expire time in memcache, setting
  a larger value gets it treated as a unix time (so when I said 90 days,
  everything expired april 1, 1970).
* Add admin function to rotate image.
* Add a message for login failure.
* Marginally less padding.
* Space between buttons.
* Add cli subcommand for precaching thumbnails.
* Add options for pidfile usage.
* Support and specify `next` parameter in login form and links.
* Update some dependencies.
* Lots of code cleanup and refactoring.


## Release 0.2.0 (2017-08-01)

Too many changes to list.


## Initial commit (2015-11-19)
    
Very simple web service using the nickel framework.