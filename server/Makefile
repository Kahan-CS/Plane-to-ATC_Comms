CC      = gcc
CFLAGS  = -Wall -Wextra -std=c11
INCLUDES = -Iinclude -I../shared

SRC     = src/main.c
BIN     = atc-server

TEST_SRCS = $(wildcard tests/*.c)

.PHONY: build test clean

build:
	$(CC) $(CFLAGS) $(INCLUDES) $(SRC) -o $(BIN)

test:
	$(CC) -Wall -Wextra -std=c11 \
		-I./include \
		-I./src/include \
		-I../shared \
		tests/test_state_machine.c \
		-o tests/test_state_machine_runner.exe
	./tests/test_state_machine_runner.exe

clean:
	rm -f $(BIN) *.o tests/*.o
