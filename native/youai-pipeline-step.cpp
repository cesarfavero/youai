#include "llama.h"

#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <unistd.h>
#include <fstream>
#include <string>
#include <vector>

static void usage(const char * argv0) {
    fprintf(stderr,
            "Usage: %s -m MODEL --session-dir DIR --op OP [options]\n"
            "  OP:\n"
            "    prefill-prompt   -p PROMPT          (stage 0: tokenize + forward prompt)\n"
            "    decode-token     --token-id ID      (stage 0: forward one token)\n"
            "    forward-activation --activation-in FILE [--sample 0|1] (stage 1+: embd forward)\n",
            argv0);
}

static bool read_file(const std::string & path, std::vector<float> & out) {
    std::ifstream in(path, std::ios::binary);
    if (!in) {
        return false;
    }
    in.seekg(0, std::ios::end);
    const auto nbytes = in.tellg();
    if (nbytes <= 0 || nbytes % sizeof(float) != 0) {
        return false;
    }
    in.seekg(0, std::ios::beg);
    out.resize(static_cast<size_t>(nbytes / sizeof(float)));
    in.read(reinterpret_cast<char *>(out.data()), nbytes);
    return static_cast<bool>(in);
}

static bool write_file(const std::string & path, const float * data, size_t n) {
    std::ofstream out(path, std::ios::binary);
    if (!out) {
        return false;
    }
    out.write(reinterpret_cast<const char *>(data), static_cast<std::streamsize>(n * sizeof(float)));
    return static_cast<bool>(out);
}

static std::string path_join(const std::string & dir, const std::string & name) {
    if (dir.empty()) {
        return name;
    }
    if (dir.back() == '/') {
        return dir + name;
    }
    return dir + "/" + name;
}

static bool load_state(llama_context * ctx, const std::string & session_dir, std::vector<llama_token> & tokens) {
    const std::string state_path = path_join(session_dir, "state.bin");
    FILE * f = fopen(state_path.c_str(), "rb");
    if (!f) {
        tokens.clear();
        return true; // fresh session
    }
    fclose(f);

    tokens.assign(4096, 0);
    size_t n_out = 0;
    const bool ok = llama_state_load_file(
        ctx, state_path.c_str(), tokens.data(), tokens.size(), &n_out);
    tokens.resize(n_out);
    return ok;
}

static bool save_state(llama_context * ctx, const std::string & session_dir, const std::vector<llama_token> & tokens) {
    const std::string state_path = path_join(session_dir, "state.bin");
    return llama_state_save_file(
        ctx, state_path.c_str(), tokens.empty() ? nullptr : tokens.data(), tokens.size());
}

static bool write_meta(const std::string & session_dir, int n_past) {
    const std::string meta_path = path_join(session_dir, "meta.txt");
    std::ofstream out(meta_path);
    if (!out) {
        return false;
    }
    out << n_past << "\n";
    return static_cast<bool>(out);
}

static bool read_meta(const std::string & session_dir, int & n_past) {
    const std::string meta_path = path_join(session_dir, "meta.txt");
    std::ifstream in(meta_path);
    if (!in) {
        n_past = 0;
        return true;
    }
    in >> n_past;
    return static_cast<bool>(in);
}

struct session_ctx {
    llama_model * model = nullptr;
    llama_context * ctx = nullptr;
    const llama_vocab * vocab = nullptr;
    std::string session_dir;
    int n_embd = 0;
    int n_past = 0;
    std::vector<llama_token> tokens;
};

static bool init_session(session_ctx & s, const std::string & model_path, const std::string & session_dir, int n_ctx) {
    ggml_backend_load_all();

    llama_model_params mparams = llama_model_default_params();
    s.model = llama_model_load_from_file(model_path.c_str(), mparams);
    if (!s.model) {
        fprintf(stderr, "failed to load model: %s\n", model_path.c_str());
        return false;
    }

    s.vocab = llama_model_get_vocab(s.model);
    s.n_embd = llama_model_n_embd_out(s.model);
    s.session_dir = session_dir;

    llama_context_params cparams = llama_context_default_params();
    cparams.n_ctx = n_ctx;
    cparams.n_batch = n_ctx;
    cparams.no_perf = true;
    cparams.embeddings = true;

    s.ctx = llama_init_from_model(s.model, cparams);
    if (!s.ctx) {
        fprintf(stderr, "failed to create context\n");
        return false;
    }
    llama_set_embeddings(s.ctx, true);

    if (!load_state(s.ctx, session_dir, s.tokens)) {
        fprintf(stderr, "failed to load session state\n");
        return false;
    }
    read_meta(session_dir, s.n_past);
    if (s.n_past == 0 && !s.tokens.empty()) {
        s.n_past = static_cast<int>(s.tokens.size());
    }
    return true;
}

static void free_session(session_ctx & s) {
    if (s.ctx) {
        llama_free(s.ctx);
        s.ctx = nullptr;
    }
    if (s.model) {
        llama_model_free(s.model);
        s.model = nullptr;
    }
}

static bool export_hidden(session_ctx & s, const std::string & out_path) {
    const float * embd = llama_get_embeddings_ith(s.ctx, -1);
    if (!embd) {
        fprintf(stderr, "no embeddings available\n");
        return false;
    }
    if (!write_file(out_path, embd, static_cast<size_t>(s.n_embd))) {
        fprintf(stderr, "failed to write activation: %s\n", out_path.c_str());
        return false;
    }
    return true;
}

static void fill_token_batch(llama_batch & batch, llama_token * tokens, int n_toks, int n_past, bool logits_last) {
    batch.n_tokens = n_toks;
    for (int i = 0; i < n_toks; ++i) {
        batch.token[i] = tokens[i];
        batch.pos[i] = n_past + i;
        batch.n_seq_id[i] = 1;
        batch.seq_id[i][0] = 0;
        batch.logits[i] = logits_last && (i == n_toks - 1);
    }
}

static bool run_prefill_prompt(session_ctx & s, const std::string & prompt, const std::string & activation_out) {
    const int n_toks = -llama_tokenize(s.vocab, prompt.c_str(), prompt.size(), nullptr, 0, true, true);
    if (n_toks <= 0) {
        fprintf(stderr, "failed to tokenize prompt\n");
        return false;
    }
    std::vector<llama_token> tokens(static_cast<size_t>(n_toks));
    if (llama_tokenize(s.vocab, prompt.c_str(), prompt.size(), tokens.data(), n_toks, true, true) < 0) {
        fprintf(stderr, "failed to tokenize prompt\n");
        return false;
    }

    llama_batch batch = llama_batch_init(n_toks, 0, 1);
    fill_token_batch(batch, tokens.data(), n_toks, s.n_past, true);

    if (llama_decode(s.ctx, batch)) {
        llama_batch_free(batch);
        fprintf(stderr, "decode failed during prefill\n");
        return false;
    }
    llama_batch_free(batch);

    s.n_past += n_toks;
    s.tokens = tokens;
    if (!export_hidden(s, activation_out)) {
        return false;
    }

    if (!save_state(s.ctx, s.session_dir, s.tokens)) {
        fprintf(stderr, "failed to save session state\n");
        return false;
    }
    write_meta(s.session_dir, s.n_past);
    printf("{\"ok\":true,\"op\":\"prefill-prompt\",\"n_past\":%d,\"n_embd\":%d}\n", s.n_past, s.n_embd);
    fflush(stdout);
    _exit(0);
}

static bool run_decode_token(session_ctx & s, llama_token token, const std::string & activation_out) {
    llama_batch batch = llama_batch_init(1, 0, 1);
    fill_token_batch(batch, &token, 1, s.n_past, true);

    if (llama_decode(s.ctx, batch)) {
        llama_batch_free(batch);
        fprintf(stderr, "decode failed for token %d\n", token);
        return false;
    }
    llama_batch_free(batch);

    s.n_past += 1;
    s.tokens.push_back(token);
    if (!export_hidden(s, activation_out)) {
        return false;
    }

    if (!save_state(s.ctx, s.session_dir, s.tokens)) {
        fprintf(stderr, "failed to save session state\n");
        return false;
    }
    write_meta(s.session_dir, s.n_past);
    printf("{\"ok\":true,\"op\":\"decode-token\",\"n_past\":%d,\"token_id\":%d}\n", s.n_past, token);
    fflush(stdout);
    _exit(0);
}

static bool run_forward_activation(session_ctx & s, const std::string & activation_in, bool sample, llama_token & sampled) {
    std::vector<float> activation;
    if (!read_file(activation_in, activation)) {
        fprintf(stderr, "failed to read activation: %s\n", activation_in.c_str());
        return false;
    }
    if (static_cast<int>(activation.size()) != s.n_embd) {
        fprintf(stderr, "activation size mismatch: got %zu, expected %d\n", activation.size(), s.n_embd);
        return false;
    }

    llama_batch batch = llama_batch_init(1, s.n_embd, 1);
    batch.n_tokens = 1;
    batch.embd = activation.data();
    batch.pos[0] = s.n_past;
    batch.n_seq_id[0] = 1;
    batch.seq_id[0][0] = 0;
    batch.logits[0] = 1;

    if (llama_decode(s.ctx, batch)) {
        llama_batch_free(batch);
        fprintf(stderr, "decode failed for activation input\n");
        return false;
    }
    llama_batch_free(batch);

    s.n_past += 1;

    if (!sample) {
        printf("{\"ok\":true,\"op\":\"forward-activation\",\"n_past\":%d}\n", s.n_past);
        fflush(stdout);
        _exit(0);
    }

    const float * logits = llama_get_logits_ith(s.ctx, -1);
    if (!logits) {
        fprintf(stderr, "no logits available\n");
        return false;
    }

    const int n_vocab = llama_vocab_n_tokens(s.vocab);
    int best_id = 0;
    float best_val = logits[0];
    for (int i = 1; i < n_vocab; ++i) {
        if (logits[i] > best_val) {
            best_val = logits[i];
            best_id = i;
        }
    }
    sampled = best_id;
    s.tokens.push_back(sampled);

    if (!save_state(s.ctx, s.session_dir, s.tokens)) {
        fprintf(stderr, "failed to save session state\n");
        return false;
    }
    write_meta(s.session_dir, s.n_past);

    char piece[128];
    int n_piece = llama_token_to_piece(s.vocab, sampled, piece, sizeof(piece), 0, true);
    std::string text;
    if (n_piece > 0) {
        text.assign(piece, static_cast<size_t>(n_piece));
    }

    printf("{\"ok\":true,\"op\":\"forward-activation\",\"n_past\":%d,\"token_id\":%d,\"text\":", s.n_past, sampled);
    // minimal JSON string escape
    putchar('"');
    for (char c : text) {
        if (c == '"' || c == '\\') {
            putchar('\\');
        }
        putchar(c);
    }
    printf("\"}\n");
    fflush(stdout);
    _exit(0);
}

int main(int argc, char ** argv) {
    std::string model_path;
    std::string session_dir;
    std::string op;
    std::string prompt;
    std::string activation_in;
    std::string activation_out;
    int token_id = -1;
    bool sample = true;
    int n_ctx = 4096;

    for (int i = 1; i < argc; ++i) {
        const char * arg = argv[i];
        if (strcmp(arg, "-m") == 0 && i + 1 < argc) {
            model_path = argv[++i];
        } else if (strcmp(arg, "--session-dir") == 0 && i + 1 < argc) {
            session_dir = argv[++i];
        } else if (strcmp(arg, "--op") == 0 && i + 1 < argc) {
            op = argv[++i];
        } else if (strcmp(arg, "-p") == 0 && i + 1 < argc) {
            prompt = argv[++i];
        } else if (strcmp(arg, "--token-id") == 0 && i + 1 < argc) {
            token_id = atoi(argv[++i]);
        } else if (strcmp(arg, "--activation-in") == 0 && i + 1 < argc) {
            activation_in = argv[++i];
        } else if (strcmp(arg, "--activation-out") == 0 && i + 1 < argc) {
            activation_out = argv[++i];
        } else if (strcmp(arg, "--sample") == 0 && i + 1 < argc) {
            sample = atoi(argv[++i]) != 0;
        } else if (strcmp(arg, "-c") == 0 && i + 1 < argc) {
            n_ctx = atoi(argv[++i]);
        } else if (strcmp(arg, "-h") == 0 || strcmp(arg, "--help") == 0) {
            usage(argv[0]);
            return 0;
        } else {
            fprintf(stderr, "unknown arg: %s\n", arg);
            usage(argv[0]);
            return 1;
        }
    }

    if (model_path.empty() || session_dir.empty() || op.empty()) {
        usage(argv[0]);
        return 1;
    }

    session_ctx s;
    if (!init_session(s, model_path, session_dir, n_ctx)) {
        return 1;
    }

    bool ok = false;
    llama_token sampled = 0;

    if (op == "prefill-prompt") {
        if (prompt.empty() || activation_out.empty()) {
            fprintf(stderr, "prefill-prompt requires -p and --activation-out\n");
        } else {
            ok = run_prefill_prompt(s, prompt, activation_out);
        }
    } else if (op == "decode-token") {
        if (token_id < 0 || activation_out.empty()) {
            fprintf(stderr, "decode-token requires --token-id and --activation-out\n");
        } else {
            ok = run_decode_token(s, static_cast<llama_token>(token_id), activation_out);
        }
    } else if (op == "forward-activation") {
        if (activation_in.empty()) {
            fprintf(stderr, "forward-activation requires --activation-in\n");
        } else {
            ok = run_forward_activation(s, activation_in, sample, sampled);
        }
    } else {
        fprintf(stderr, "unknown op: %s\n", op.c_str());
    }

    if (ok) {
        fflush(stdout);
        _exit(0);
    }

    free_session(s);
    return 1;
}