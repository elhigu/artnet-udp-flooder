# Art-Net Hub UDP flooder

Server which sends art-net packets to configured hosts 40 fps speed at most 2 packets / ms.

## Getting started

    cargo build
    cargo run

## Configuration

    # TODO: cleanup all unused configs

    {
        "listen": {
            "//": "Not used for anything... reminder of other project",
            "address": "0.0.0.0",
            "port": 6454
        },
        "mappings": [
            {
            "host": { "address": "192.168.0.11", "port": 6454 },
            "//": "Only input range matters, it is number of universes that we send to given host",
            "universes": { "input": [0, 31], "output_start": 0 }
            }
        ]
    }
