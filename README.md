# rphotos

Semi-experimental web app in Rust language: manage my photos.

[![CI](https://github.com/kaj/rphotos/workflows/CI/badge.svg)](https://github.com/kaj/rphotos/actions)

Tag photos with places, people and other tags, while keeping some
private and making others public.
It uses a postgresql database for the metadata, and works with
read-only access to the actual image files.
Downscaled images are stored in memcache.

My images are on [img.krats.se](https://img.krats.se/) where you can
see those that are public though rphotos.

Not in any way feature-complete, but useful.  At least to me.

There is not (yet) much documentation, but there is command line help
(the single binary has subcommands for running the server and some
administrative task, such a finding new photos or making photos
public).
The database is described in the migrations.
Everything else is in the code.
