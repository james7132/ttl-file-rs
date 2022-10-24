# ttl-file-rs

This is a simple daemon that monitors directories and deletes files
that have exceeded a configured
[time-to-live](https://en.wikipedia.org/wiki/Time_to_live).

## Usage

```bash
ttl-file <ROOT DIRECTORIES>
```

The daemon will watch all provided directories recursively and delete files based
on a per-directory TTL. If no directories are provided, the current working
directory will be used instead.

TTL is configured by including directories in the filepath named with the pattern
`ttl=<DURATION>`. The TTL is based on the creation timestamp, not the last
modified timestamp. For example, `/home/user/directory/ttl=30d/documents/test.pdf`
will be deleted 30 days after the file is first created. TTLs are enforced down
to the precision of one second.

If multiple `ttl=X` directories are found on the filepath, the lowest-level
directory's configuration will be used For example,
`/home/user/directory/ttl=30d/documents/ttl=30m/test.pdf` will use a 30 minute
TTL instead of the upper level 30 day config.

Only files will be deleted. Directories will not be touched.

Filesystems with hard-linked loops will likely cause the daemon to infinitely
loop. This should generally not be possible with most modern Unix-based systems.

## License
MIT or Apache 2.0
