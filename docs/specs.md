# Basic Archive Format

- [Header](#header) (256 bytes)
- First [file table segment](#file-table-segment)
- Blobs (content of files)

The names and blobs section are organized in a sequential manner ; they are completely unstructured.

## Header

- Magic number (8 bytes): ASCII-encoded `BASICARC`
- Archive version (4 bytes), little endian, `0x1` is the very first version
- _Future-proof_ (250 bytes), filled with zeroes

## File table

The file table is made of one or several [segments](#file-table-segment), that may not be located one after the other.

### File table segment

- Address of the next segment (8 bytes): `0` if none
- Maximum number of directories (4 bytes)
- Maximum number of files in the segment (4 bytes)
- For each directory:
    - Archive-unique ID (8 bytes): `0` for removed entries
    - Parent directory ID (8 bytes): `0` if none
    - Length of the name in bytes (1 byte)
    - UTF-8-encoded name (255 bytes)
    - Modification time (8 bytes)
- For each file:
    - Archive-unique ID (8 bytes): `0` for removed entries
    - Parent directory ID (8 bytes): `0` if none
    - Length of the name in bytes (1 byte)
    - UTF-8-encoded name (255 bytes)
    - Modification time (8 bytes)
    - Address of the content (8 bytes)
    - Length of the content (8 bytes)
    - SHA-3 checksum of the content (32 bytes)
