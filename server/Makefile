CC = gcc
CFLAGS = -Wall -Wextra -std=c11
INCLUDES = -I./include -I./src/include -I../shared

SRC = src/main.c
BIN = atc-server.exe

UNIT_TEST_SRCS = \
	tests/unittests/test_state_machine.c \
	tests/unittests/test_logger.c \
	tests/unittests/test_packet.c

UNIT_TEST_BINS = \
	tests/bin/test_state_machine.exe \
	tests/bin/test_logger.exe \
	tests/bin/test_packet.exe

COV_TEST_BINS = \
	tests/bin_cov/test_state_machine_cov.exe \
	tests/bin_cov/test_logger_cov.exe \
	tests/bin_cov/test_packet_cov.exe

.PHONY: build test unit integration regression coverage coverage-clean clean

build:
	$(CC) $(CFLAGS) $(INCLUDES) $(SRC) -o $(BIN) -lws2_32

tests/bin/test_state_machine.exe: tests/unittests/test_state_machine.c
	@if not exist tests\\bin mkdir tests\\bin
	$(CC) $(CFLAGS) $(INCLUDES) $< -o $@

tests/bin/test_logger.exe: tests/unittests/test_logger.c
	@if not exist tests\\bin mkdir tests\\bin
	$(CC) $(CFLAGS) $(INCLUDES) $< -o $@

tests/bin/test_packet.exe: tests/unittests/test_packet.c
	@if not exist tests\\bin mkdir tests\\bin
	$(CC) $(CFLAGS) $(INCLUDES) $< -o $@

unit: $(UNIT_TEST_BINS)
	.\\tests\\bin\\test_state_machine.exe
	.\\tests\\bin\\test_logger.exe
	.\\tests\\bin\\test_packet.exe

test: unit

integration: build
	powershell -ExecutionPolicy Bypass -File tests/integration/test_accept_loop.ps1 -ServerExe ./$(BIN)

regression: unit integration

tests/bin_cov/test_state_machine_cov.exe: tests/unittests/test_state_machine.c
	@if not exist tests\\bin_cov mkdir tests\\bin_cov
	$(CC) $(CFLAGS) --coverage $(INCLUDES) $< -o $@

tests/bin_cov/test_logger_cov.exe: tests/unittests/test_logger.c
	@if not exist tests\\bin_cov mkdir tests\\bin_cov
	$(CC) $(CFLAGS) --coverage $(INCLUDES) $< -o $@

tests/bin_cov/test_packet_cov.exe: tests/unittests/test_packet.c
	@if not exist tests\\bin_cov mkdir tests\\bin_cov
	$(CC) $(CFLAGS) --coverage $(INCLUDES) $< -o $@

coverage-clean:
	@if exist tests\\bin\\*.exe del /q tests\\bin\\*.exe
	@if exist tests\\bin_cov\\*.exe del /q tests\\bin_cov\\*.exe
	@if exist tests\\bin_cov\\*.gcno del /q tests\\bin_cov\\*.gcno
	@if exist tests\\bin_cov\\*.gcda del /q tests\\bin_cov\\*.gcda
	@if exist tests\\unittests\\*.gcno del /q tests\\unittests\\*.gcno
	@if exist tests\\unittests\\*.gcda del /q tests\\unittests\\*.gcda
	@if exist *.gcno del /q *.gcno
	@if exist *.gcda del /q *.gcda
	@if exist coverage_report rmdir /s /q coverage_report

coverage: coverage-clean $(COV_TEST_BINS)
	.\\tests\\bin_cov\\test_state_machine_cov.exe
	.\\tests\\bin_cov\\test_logger_cov.exe
	.\\tests\\bin_cov\\test_packet_cov.exe
	@if not exist coverage_report mkdir coverage_report
	python -m gcovr \
		--root . \
		--filter src/include \
		--filter ../shared \
		--exclude tests \
		--print-summary \
		--txt coverage_report/summary.txt \
		--html-details coverage_report/index.html

clean: coverage-clean
	@if exist $(BIN) del /q $(BIN)
	@if exist *.o del /q *.o
