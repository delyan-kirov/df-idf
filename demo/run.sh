#! /usr/bin/bash

# rebuild server
tsc
# run in dev mode
nodemon --exec ts-node src/server.ts &
# open page
firefox http://localhost:3000
