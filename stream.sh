#!/bin/bash

# see stream-credentials.env.example for the type of file to use here
source "$@"

python3 radio.py radio -o "ffmpegre:-acodec libmp3lame -ab 300k -content_type audio/mpeg -f mp3 icecast://source:$STREAM_PW@$STREAM_HOST/$STREAM_MOUNT" -m "$STREAM_SCHEMA://source:$STREAM_PW@$STREAM_HOST/admin/metadata?mount=%2F$STREAM_MOUNT&mode=updinfo" $STREAM_FILES
