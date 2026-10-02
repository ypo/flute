/*
 * Prints the repair symbols computed by a reference Reed-Solomon implementation,
 * to check the compatibility of src/fec/rsgf2m.rs. See run.sh.
 *
 * Build with -DWITH_ZFEC (and zfec/fec.c) or -DWITH_OPENFEC (and -lopenfec).
 *
 * Usage: rs_vectors <codec> <m> <k> <n> <E>
 *   codec: zfec, openfec_rs_2_8 or openfec_rs_2_m
 *
 * Source byte j of the block (j = 0 .. k * E - 1) is (j * 31 + j / 7) mod 256.
 * Output, one line per repair symbol: <codec> <m> <k> <n> <E> <esi> <hex>
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#ifdef WITH_ZFEC
#include "fec.h"
#endif

#ifdef WITH_OPENFEC
#include "of_openfec_api.h"
#endif

static int encode(const char *codec, unsigned m, unsigned k, unsigned n, unsigned e,
                  unsigned char **symbols)
{
#ifdef WITH_ZFEC
    if (strcmp(codec, "zfec") == 0) {
        if (m != 8)
            return -1;
        unsigned nb_repair = n - k;
        unsigned *block_nums = malloc(nb_repair * sizeof(unsigned));
        for (unsigned i = 0; i < nb_repair; i++)
            block_nums[i] = k + i;
        fec_init();
        fec_t *code = fec_new(k, n);
        fec_encode(code, (const gf *const *)symbols, (gf *const *)(symbols + k), block_nums,
                   nb_repair, e);
        fec_free(code);
        free(block_nums);
        return 0;
    }
#endif

#ifdef WITH_OPENFEC
    of_rs_parameters_t rs_2_8 = {0};
    of_rs_2_m_parameters_t rs_2_m = {0};
    of_codec_id_t codec_id;
    of_parameters_t *params;
    if (strcmp(codec, "openfec_rs_2_8") == 0 && m == 8) {
        codec_id = OF_CODEC_REED_SOLOMON_GF_2_8_STABLE;
        rs_2_8.nb_source_symbols = k;
        rs_2_8.nb_repair_symbols = n - k;
        rs_2_8.encoding_symbol_length = e;
        params = (of_parameters_t *)&rs_2_8;
    } else if (strcmp(codec, "openfec_rs_2_m") == 0) {
        codec_id = OF_CODEC_REED_SOLOMON_GF_2_M_STABLE;
        rs_2_m.nb_source_symbols = k;
        rs_2_m.nb_repair_symbols = n - k;
        rs_2_m.encoding_symbol_length = e;
        rs_2_m.m = m;
        params = (of_parameters_t *)&rs_2_m;
    } else {
        return -1;
    }

    of_session_t *session = NULL;
    if (of_create_codec_instance(&session, codec_id, OF_ENCODER, 0) != OF_STATUS_OK)
        return -1;
    if (of_set_fec_parameters(session, params) != OF_STATUS_OK)
        return -1;
    for (unsigned esi = k; esi < n; esi++) {
        if (of_build_repair_symbol(session, (void **)symbols, esi) != OF_STATUS_OK)
            return -1;
    }
    of_release_codec_instance(session);
    return 0;
#endif

    return -1;
}

int main(int argc, char **argv)
{
    if (argc != 6) {
        fprintf(stderr, "usage: %s <codec> <m> <k> <n> <E>\n", argv[0]);
        return 2;
    }
    const char *codec = argv[1];
    unsigned m = atoi(argv[2]), k = atoi(argv[3]), n = atoi(argv[4]), e = atoi(argv[5]);
    if (k == 0 || n < k || e == 0) {
        fprintf(stderr, "invalid parameters\n");
        return 2;
    }

    unsigned char **symbols = calloc(n, sizeof(unsigned char *));
    for (unsigned i = 0; i < n; i++)
        symbols[i] = calloc(e, 1);
    for (unsigned j = 0; j < k * e; j++)
        symbols[j / e][j % e] = (unsigned char)(j * 31 + j / 7);

    if (encode(codec, m, k, n, e, symbols) != 0) {
        fprintf(stderr, "%s: cannot encode m=%u k=%u n=%u E=%u\n", codec, m, k, n, e);
        return 1;
    }

    for (unsigned esi = k; esi < n; esi++) {
        printf("%s %u %u %u %u %u ", codec, m, k, n, e, esi);
        for (unsigned j = 0; j < e; j++)
            printf("%02x", symbols[esi][j]);
        printf("\n");
    }

    for (unsigned i = 0; i < n; i++)
        free(symbols[i]);
    free(symbols);
    return 0;
}
