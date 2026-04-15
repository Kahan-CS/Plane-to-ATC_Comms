CC      = gcc
CFLAGS  = -Wall -Wextra -std=c11
INCLUDES = -Iinclude -I../shared

SRC     = src/main.c
BIN     = atc-server

TEST_SRCS = $(wildcard tests/*.c)

.PHONY: build test coverage coverage-clean clean

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

coverage:
	$(CC) -Wall -Wextra -std=c11 --coverage \
		-I./include \
		-I./src/include \
		-I../shared \
		tests/test_state_machine.c \
		-o tests/test_state_machine_cov.exe
	$(CC) -Wall -Wextra -std=c11 --coverage \
		-I./include \
		-I./src/include \
		-I../shared \
		tests/test_logger.c \
		-o tests/test_logger_cov.exe
	$(CC) -Wall -Wextra -std=c11 --coverage \
		-I./include \
		-I./src/include \
		-I../shared \
		tests/test_packet.c \
		-o tests/test_packet_cov.exe
	./tests/test_state_machine_cov.exe
	./tests/test_logger_cov.exe
	./tests/test_packet_cov.exe
	mkdir -p coverage_report
	python -m gcovr \
		--root . \
		--filter src/include/ \
		--filter ../shared/ \
		--html-details coverage_report/index.html \
		--txt coverage_report/summary.txt \
		--print-summary

coverage-clean:
	rm -f tests/*_cov.exe
	rm -f tests/*.gcno tests/*.gcda
	rm -f *.gcno *.gcda
	rm -rf coverage_report/

integration:
	@echo "Starting server in background..."
	@echo "Run: python -m pytest integration_tests/ -v"
	@echo "Make sure atc-server.exe is running on port 9000"

clean:
	rm -f $(BIN) *.o tests/*.o
