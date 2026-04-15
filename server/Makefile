CC      = gcc
CFLAGS  = -Wall -Wextra -std=c11
INCLUDES = -Iinclude -I../shared

SRC     = src/main.c
BIN     = atc-server

TEST_SRCS = $(wildcard tests/*.c)

.PHONY: build test clean

build:
	$(CC) $(CFLAGS) $(INCLUDES) $(SRC) -o $(BIN) -lws2_32

test:
ifeq ($(TEST_SRCS),)
	@echo "No test files yet."
else
	@for t in $(TEST_SRCS); do \
		bin=$$(basename $$t .c); \
		$(CC) $(CFLAGS) $(INCLUDES) $$t -o $$bin && ./$$bin; \
	done
endif

integration:
	@echo "Starting server in background..."
	@echo "Run: python -m pytest integration_tests/ -v"
	@echo "Make sure atc-server.exe is running on port 9000"

clean:
	rm -f $(BIN) *.o tests/*.o
