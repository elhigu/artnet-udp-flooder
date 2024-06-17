# Art-Net UDP flooder

Server which sends art-net packets to configured hosts at most 2 packets / ms.

## Getting started

    cargo build
    cargo run

## Configuration

    {
        "listen": {
            "//": "Not really used for anything",
            "address": "0.0.0.0",
            "port": 6666
        },
        "//": "How often to trigger new output frame sending",
        "fps": 40.0,
        "outputs": [
                {
                "host": {
                    "address": "192.168.137.23",
                    "port": 6454
                },
                "//": "number of microseconds to wait before sending next packet to the host",
                "throttle_us": 200,
                "//": "Number of universes to send to output",
                "universe_count": 8
            }
        ]
    }
