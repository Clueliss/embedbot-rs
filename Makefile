SOURCE_PREFIX ::= ./
DATA_PREFIX ::= ./config
UID != id -u liss
GID != id -g liss

build:
	podman image build \
			--tag embedbot:latest \
			${SOURCE_PREFIX}

up: build
	podman container run --rm --net=host -it \
    			--name embedbot \
    			--user ${UID}:${GID} \
    			-v ${DATA_PREFIX}/embedbot.toml:/etc/embedbot.toml:ro \
    			embedbot:latest
