#include "Valu.h"
#include "Valu___024unit.h"
#include "verilated_snapshot.hpp"
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <iostream>
#include <stdlib.h>
#include <vector>
#include <verilated.h>
#include <verilated_save.h>
#include <verilated_vcd_c.h>

#define WARMUP_SIM_TIME 20

void warmup(Valu *dut) {
    vluint64_t sim_time = 0;

    dut->rst = 0;

    while (sim_time < WARMUP_SIM_TIME) {
        if (sim_time == 2) {
            dut->rst = 1;
        }

        dut->clk ^= 1;
        dut->eval();
        sim_time++;
    }
}

typedef struct {
    VerilatedSnapshot *snapshot;
    int num_loops;
    int num_cycles;
} simulation_context_t;

void *simulate(void *params) {
    simulation_context_t *ctx = (simulation_context_t *)params;

    VerilatedSnapshot snapshot(*ctx->snapshot);

    Valu *dut = new Valu;

    VerilatedSnapshotRestore restore(&snapshot);
    int num_loops = ctx->num_loops;
    int num_cycles = ctx->num_cycles;

    for (int i = 0; i < num_loops; i++) {
        vluint64_t sim_time = WARMUP_SIM_TIME;
        restore >> *dut;

        while (sim_time < (num_cycles + WARMUP_SIM_TIME)) {
            dut->clk ^= 1;
            dut->eval();
            sim_time++;
        }

        restore.reset();
    }

    free(dut);

    return NULL;
}

int main(int argc, char **argv, char **env) {
    int num_threads, num_loops, num_cycles;

    if (argc < 4) {
        fprintf(stderr, "Usage: %s <NUM THREADS> <NUM LOOPS> <NUM CYCLES>\n",
                argv[0]);
        exit(2);
    }

    num_threads = atoi(argv[1]);
    num_loops = atoi(argv[2]);
    num_cycles = atoi(argv[3]);

    if (!num_threads || !num_loops || !num_cycles) {
        fprintf(stderr, "Threads, loops or cycles cannot be 0\n");
        exit(2);
    }

    Valu *dut = new Valu;

    VerilatedSnapshot snapshot;

    warmup(dut);

    snapshot << *dut;

    pthread_t *threads = (pthread_t *)malloc(sizeof(pthread_t) * num_threads);
    simulation_context_t *ctxs = (simulation_context_t *)malloc(
        sizeof(simulation_context_t) * num_threads);

    for (int i = 0; i < num_threads; i++) {
        ctxs[i].snapshot = &snapshot;
        ctxs[i].num_loops = num_loops;
        ctxs[i].num_cycles = num_cycles;

        int rv = pthread_create(&threads[i], NULL, &simulate, &ctxs[i]);

        if (rv != 0) {
            fprintf(stderr, "Failed to create thread\n");
            exit(1);
        }
    }

    for (int i = 0; i < num_threads; i++) {
        pthread_join(threads[i], NULL);
    }

    delete dut;
    exit(EXIT_SUCCESS);
}