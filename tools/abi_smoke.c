/* ArchType C ABI 冒烟测试：模拟 fcitx5 插件的调用序列。
 *
 * 覆盖：引擎创建 -> 输入 -> 取候选 -> 记录选词 -> 设置上下文 -> 预测下一词
 *      -> 用户词增删 -> 用户词库持久化。
 */
#include <stdio.h>
#include <string.h>
#include "archtype_core.h"

static int failures = 0;

static void check(int cond, const char *what) {
    if (!cond) {
        failures++;
        printf("FAIL %s", what);
        putchar(10);
    } else {
        printf("ok   %s", what);
        putchar(10);
    }
}

static int has_candidate(ArchtypeCandidates *c, const char *text) {
    size_t n = archtype_candidates_len(c);
    for (size_t i = 0; i < n; i++) {
        const char *t = archtype_candidates_text(c, i);
        if (t && strcmp(t, text) == 0) return 1;
    }
    return 0;
}

int main(int argc, char **argv) {
    const char *lexicon = argc > 1 ? argv[1] : "/tmp/archtype_abi.tsv";
    const char *bigram = argc > 2 ? argv[2] : "";
    ArchtypeEngineHandle *e = archtype_engine_new(lexicon, bigram);
    if (!e) {
        printf("engine_new failed: %s", archtype_last_error());
        putchar(10);
        return 1;
    }

    ArchtypeCandidates *c = archtype_engine_input(e, "nihao");
    check(c && has_candidate(c, "你好"), "exact: nihao -> 你好");
    archtype_candidates_free(c);

    c = archtype_engine_input(e, "nih");
    check(c && has_candidate(c, "你好"), "partial: nih -> 你好");
    archtype_candidates_free(c);

    c = archtype_engine_input(e, "hello");
    check(c && has_candidate(c, "hello"), "english: hello -> hello");
    archtype_candidates_free(c);

    /* 句首词候选：consumed 小于输入长度，剩下的部分留给下一轮 */
    c = archtype_engine_input(e, "nihaozhongguo");
    {
        int found = 0;
        size_t n = archtype_candidates_len(c);
        for (size_t i = 0; c && i < n; i++) {
            const char *t = archtype_candidates_text(c, i);
            if (t && strcmp(t, "你好") == 0 && archtype_candidates_consumed(c, i) == 5) found = 1;
        }
        check(found, "prefix: nihaozhongguo 有 你好(用掉 5 字节)");
        check(c && archtype_candidates_consumed(c, 0) == 13, "整句候选用掉全部输入");
        const char *py = c ? archtype_candidates_pinyin(c, 0) : NULL;
        check(py && strcmp(py, "ni hao zhong guo") == 0, "candidates_pinyin");
    }
    archtype_candidates_free(c);

    /* 按错键：zhonggup（o 旁边的 p） */
    c = archtype_engine_input(e, "zhonggup");
    check(c && has_candidate(c, "中国"), "typo: zhonggup -> 中国");
    archtype_candidates_free(c);
    check(archtype_engine_set_typo_correction(e, 0) == 0, "set_typo_correction");
    c = archtype_engine_input(e, "zhonggup");
    check(c && !has_candidate(c, "中国"), "纠错关闭后 zhonggup 不出 中国");
    archtype_candidates_free(c);
    archtype_engine_set_typo_correction(e, 1);
    check(archtype_engine_set_max_candidates(e, 5) == 0, "set_max_candidates");

    check(archtype_engine_record_selection(e, "你好") == 0, "record_selection");
    check(archtype_engine_set_context(e, "今天") == 0, "set_context");

    c = archtype_engine_input(e, "tian");
    check(c && has_candidate(c, "天"), "context: tian -> 天");
    archtype_candidates_free(c);

    c = archtype_engine_predict_next(e, "今天", 3);
    check(c && archtype_candidates_len(c) > 0, "predict_next 有结果");
    archtype_candidates_free(c);

    /* 领域模型：设两次是换掉不是叠着混；混上以后联想照样按通用模型；设错的路径不动原来的 */
    if (*bigram) {
        check(archtype_engine_set_domain_lm(e, bigram) == 0, "set_domain_lm");
        check(archtype_engine_set_domain_lm(e, bigram) == 0, "set_domain_lm 再设一次");
        c = archtype_engine_predict_next(e, "今天", 3);
        check(c && archtype_candidates_len(c) > 0, "混上领域模型后 predict_next 有结果");
        archtype_candidates_free(c);
    }
    check(archtype_engine_set_domain_lm(e, "/nonexistent/words.domain.lm") == -1, "set_domain_lm 文件不在返回 -1");
    check(archtype_engine_set_domain_lm(e, NULL) == 0, "set_domain_lm(NULL) 去掉");

    check(archtype_engine_add_user_word(e, "wo wo te", "我特") == 0, "add_user_word");
    c = archtype_engine_input(e, "wowote");
    check(c && has_candidate(c, "我特"), "user word 可输入");
    archtype_candidates_free(c);

    check(archtype_engine_set_fuzzy(e, 1) == 0, "set_fuzzy");
    c = archtype_engine_input(e, "zidao");
    check(c && has_candidate(c, "知道"), "fuzzy: zidao -> 知道");
    archtype_candidates_free(c);

    check(archtype_engine_set_shuangpin(e, 1) == 0, "set_shuangpin");
    c = archtype_engine_input(e, "vsgo");
    check(c && has_candidate(c, "中国"), "shuangpin: vsgo -> 中国");
    archtype_candidates_free(c);
    archtype_engine_set_shuangpin(e, 0);

    const char *s2t = argc > 3 ? argv[3] : "";
    if (*s2t) {
        check(archtype_engine_load_conversion(e, s2t) > 0, "load_conversion");
        check(archtype_engine_set_traditional(e, 1) == 0, "set_traditional");
        c = archtype_engine_input(e, "zhongguo");
        check(c && has_candidate(c, "中國"), "traditional: zhongguo -> 中國");
        archtype_candidates_free(c);
        archtype_engine_set_traditional(e, 0);
    }

    check(archtype_engine_save_user_dict(e, "/tmp/archtype_abi_user.tsv") == 0, "save_user_dict");
    check(archtype_engine_remove_user_word(e, "我特") == 1, "remove_user_word");
    check(archtype_engine_load_user_dict(e, "/tmp/archtype_abi_user.tsv") >= 1, "load_user_dict");
    {
        FILE *custom = fopen("/tmp/archtype_abi_custom.tsv", "w");
        check(custom != NULL, "open custom.tsv");
        fputs("ni hao\t妮好\nnot pinyin zz\t坏\n", custom);
        fclose(custom);
        check(archtype_engine_load_custom_words(e, "/tmp/archtype_abi_custom.tsv") == 1,
              "load_custom_words");
        check(archtype_engine_load_custom_words(e, "/tmp/archtype_abi_no_such.tsv") == 0,
              "load_custom_words missing file clears");
        remove("/tmp/archtype_abi_custom.tsv");
    }

    /* 自动学习：同一个词选 3 次后应当能直接打出来 */
    for (int i = 0; i < 3; i++) {
        archtype_engine_record_selection_ex(e, "阿特", "a te");
    }
    c = archtype_engine_input(e, "ate");
    check(c && has_candidate(c, "阿特"), "auto learn: ate -> 阿特");
    archtype_candidates_free(c);

    c = archtype_symbol_search("juhao", 3);
    check(c && archtype_candidates_len(c) > 0 &&
              strcmp(archtype_candidates_text(c, 0), "。") == 0,
          "symbol: juhao -> 。");
    archtype_candidates_free(c);
    {
        const char *p = archtype_fullwidth_punct(',');
        check(p && strcmp(p, "，") == 0, "punct: comma -> ，");
    }

    /* 输入会话（ABI 1.3）：字母进编辑区、退格、切换输入法时上屏。
     * 不带引擎（词库还没载入好时就是这样）：编辑区是原始输入 */
    {
        ArchtypeSession *s = archtype_session_new(NULL, NULL);
        check(s != NULL, "session_new");
        const ArchtypeFrame *f = NULL;
        const char *keys = "nihaoma";
        int letters_ok = 1;
        for (const char *k = keys; *k; ++k) {
            ArchtypeKey key = {sizeof(ArchtypeKey), (uint32_t)*k, 0, 0, 0};
            f = archtype_session_key(s, &key);
            letters_ok = letters_ok && f && f->handled && f->size == sizeof(ArchtypeFrame);
            ArchtypeKey up = key;
            up.release = 1;
            letters_ok = letters_ok && !archtype_session_key(s, &up)->handled;
        }
        check(letters_ok, "session: letters handled, releases not, frame size matches header");
        /* f 指向会话自己的 frame：上面的松开键已经覆盖了它，这里看的是松开后的编辑区 */
        check(f && strcmp(f->preedit, "nihaoma") == 0 && f->commit == NULL, "session: preedit nihaoma");
        f = archtype_session_choose(s, 0);
        check(f && !f->handled && f->commit == NULL, "session: choose without engine");
        ArchtypeKey bs = {sizeof(ArchtypeKey), 0xff08, 0, 0, 0};
        f = archtype_session_key(s, &bs);
        check(f && f->handled && strcmp(f->preedit, "nihaom") == 0, "session: backspace");
        /* ABI 1.6：没有候选时空格上屏原始输入 */
        ArchtypeKey space = {sizeof(ArchtypeKey), ' ', 0, 0, 0};
        f = archtype_session_key(s, &space);
        check(f && f->handled && f->commit && strcmp(f->commit, "nihaom") == 0 && f->preedit[0] == 0,
              "session: space commits raw input without candidates");
        for (const char *k = "ab"; *k; ++k) {
            ArchtypeKey key = {sizeof(ArchtypeKey), (uint32_t)*k, 0, 0, 0};
            archtype_session_key(s, &key);
        }
        f = archtype_session_reset(s, 1);
        check(f && f->commit && strcmp(f->commit, "ab") == 0 && f->preedit[0] == 0,
              "session: reset commits raw input");
        archtype_session_free(s);
    }

    /* 输入会话带引擎（ABI 1.4）：候选列表和分音节 */
    {
        ArchtypeSessionConfig config = {sizeof(ArchtypeSessionConfig), 9, 1, 0, 0};
        ArchtypeSession *s = archtype_session_new(NULL, &config);
        for (const char *k = "nihaozhongguo"; *k; ++k) {
            ArchtypeKey key = {sizeof(ArchtypeKey), (uint32_t)*k, 0, 0, 0};
            archtype_session_key(s, &key);
        }
        check(archtype_session_candidate_total(s) == 0, "session: no engine, no candidates");
        archtype_session_set_engine(s, e);
        const ArchtypeFrame *f = archtype_session_frame(s);
        check(f && !f->handled && strcmp(f->preedit, "ni'hao'zhong'guo") == 0,
              "session: preedit segmented");
        check(archtype_session_candidate_total(s) > 0, "session: candidates with engine");
        ArchtypeCandidateInfo info;
        memset(&info, 0, sizeof info);
        info.size = sizeof info;
        check(archtype_session_candidate_info(s, 0, &info) == 0 && strcmp(info.text, "你好中国") == 0 &&
                  strcmp(info.pinyin, "ni hao zhong guo") == 0 && info.consumed == 13 &&
                  info.kind == ARCHTYPE_CAND_WORD && strcmp(info.commit, "你好中国") == 0 &&
                  info.emoji[0] == 0 && info.size == sizeof info,
              "session: candidate info");
        check(archtype_session_candidate_info(s, archtype_session_candidate_total(s), &info) == -1,
              "session: candidate info out of range");
        /* ABI 1.5：选句首词进已选栈，剩下的接着选；选完整句上屏 */
        size_t nihao = archtype_session_candidate_total(s);
        for (size_t i = 0; i < archtype_session_candidate_total(s); ++i) {
            if (archtype_session_candidate_info(s, i, &info) == 0 && strcmp(info.text, "你好") == 0) {
                nihao = i;
                break;
            }
        }
        check(nihao < archtype_session_candidate_total(s) && info.consumed == 5, "session: 你好 candidate");
        f = archtype_session_choose(s, nihao);
        check(f && f->handled && f->commit == NULL && strcmp(f->preedit, "你好zhong'guo") == 0,
              "session: choose sentence-start word");
        check(archtype_session_candidate_info(s, 0, &info) == 0 && strcmp(info.text, "中国") == 0 &&
                  info.consumed == 8,
              "session: candidates follow selection");
        f = archtype_session_choose(s, 0);
        check(f && f->handled && f->commit && strcmp(f->commit, "你好中国") == 0 && f->preedit[0] == 0,
              "session: choose commits whole sentence");
        for (const char *k = "nihaozh"; *k; ++k) {
            ArchtypeKey key = {sizeof(ArchtypeKey), (uint32_t)*k, 0, 0, 0};
            archtype_session_key(s, &key);
        }
        ArchtypeKey enter = {sizeof(ArchtypeKey), 0xff0d, 0, 0, 0};
        f = archtype_session_key(s, &enter);
        check(f && f->commit && strcmp(f->commit, "nihaozh") == 0 && f->learned,
              "session: return commits raw letters (learned)");
        /* ABI 1.6：显示的这一页、点候选、上屏的是哪个候选、候选框状态 */
        for (const char *k = "nihao"; *k; ++k) {
            ArchtypeKey key = {sizeof(ArchtypeKey), (uint32_t)*k, 0, 0, 0};
            archtype_session_key(s, &key);
        }
        f = archtype_session_frame(s);
        const size_t shown = f ? f->cand_count : 0;
        check(shown > 0 && shown <= 9 && f->cand_cursor == 0 && f->panel == 0 && !f->has_prev,
              "session: frame shows a page");
        check(archtype_session_candidate(s, shown) == NULL, "session: candidate out of range");
        char first[64] = {0};
        const char *text = archtype_session_candidate(s, 0);
        check(text != NULL, "session: candidate text");
        if (text) {
            strncpy(first, text, sizeof first - 1);
        }
        ArchtypeSessionView view;
        memset(&view, 0, sizeof view);
        view.size = sizeof view;
        check(archtype_session_view(s, &view) == 0 && view.page == 0 && view.cursor == 0 &&
                  !view.expanded && view.size == sizeof view,
              "session: view");
        check(!archtype_session_select(s, shown)->handled, "session: select out of range");
        f = archtype_session_select(s, 0);
        check(f && f->handled && f->commit && strcmp(f->commit, first) == 0, "session: select commits");
        check(f->learned && f->mode == ARCHTYPE_MODE_CHINESE && !f->translating,
              "session: 1.10 frame fields");
        /* ABI 1.10：没在打字时标点由会话换成中文标点 */
        ArchtypeKey comma = {sizeof(ArchtypeKey), ',', 0, 0, 0};
        f = archtype_session_key(s, &comma);
        check(f && f->handled && f->commit && strcmp(f->commit, "，") == 0 && !f->learned,
              "session: punctuation");
        /* ABI 1.10：设置里的切换键、进出「译」的键由外壳认出来，带上动作交给会话 */
        ArchtypeKey toggle = {sizeof(ArchtypeKey), ' ', ARCHTYPE_MOD_CTRL | ARCHTYPE_MOD_SHIFT, 0, 0,
                             ARCHTYPE_KEY_ACTION_TOGGLE_MODE};
        f = archtype_session_key(s, &toggle);
        check(f && f->handled && f->mode_changed && f->mode == ARCHTYPE_MODE_ENGLISH, "session: toggle key");
        toggle.action = ARCHTYPE_KEY_ACTION_TOGGLE_TRANSLATE;
        f = archtype_session_key(s, &toggle);
        check(f && !f->handled && !f->mode_changed, "session: no translate, translate key passes through");
        archtype_session_set_mode(s, ARCHTYPE_MODE_CHINESE);
        archtype_session_set_engine(s, NULL);
        check(archtype_session_candidate_total(s) == 0, "session: engine unset");
        archtype_session_free(s);
    }

    /* ABI 1.10：五笔码表、会话的五笔设置、候选后面的编码提示（comment）、空码提示（aux） */
    {
        const char *path = "/tmp/archtype_abi_wubi.tsv";
        FILE *table = fopen(path, "w");
        check(table != NULL, "wubi: write table");
        if (table) {
            fputs("w\t\xE4\xBA\xBA\t100\nwq\t\xE4\xBD\xA0\t90\n"               /* 人 你 */
                  "wqvb\t\xE4\xBD\xA0\xE5\xA5\xBD\t80\nwqvb\t\xE6\x82\xA8\xE5\xA5\xBD\t70\n", /* 你好 您好 */
                  table);
            fclose(table);
        }
        check(archtype_engine_load_wubi(e, "/nonexistent/wubi86.tsv") == -1, "wubi: missing table is -1");
        check(archtype_engine_load_wubi(e, path) == 4, "wubi: load table");
        ArchtypeSessionConfig config;
        memset(&config, 0, sizeof config);
        config.size = sizeof config;
        config.page_size = 9;
        config.wubi = 1;
        config.wubi_auto_commit = 1;
        config.wubi_hint = 1;
        ArchtypeSession *s = archtype_session_new(e, &config);
        const char *keys = "wq";
        for (const char *k = keys; *k; k++) {
            ArchtypeKey key = {sizeof(ArchtypeKey), (uint32_t)*k, 0, 0, 0};
            archtype_session_key(s, &key);
        }
        ArchtypeCandidateInfo info;
        memset(&info, 0, sizeof info);
        info.size = sizeof info;
        check(archtype_session_candidate_info(s, 0, &info) == 0 && strcmp(info.text, "\xE4\xBD\xA0") == 0 &&
                  info.comment && info.comment[0] == 0,
              "wubi: exact candidate has no comment");
        check(archtype_session_candidate_info(s, 1, &info) == 0 && strcmp(info.comment, "vb") == 0,
              "wubi: longer code shows the rest");
        ArchtypeKey x = {sizeof(ArchtypeKey), 'x', 0, 0, 0};
        const ArchtypeFrame *f = archtype_session_key(s, &x);
        check(f && f->cand_count == 0 && f->aux && strcmp(f->aux, "\xE7\xA9\xBA\xE7\xA0\x81") == 0, /* 空码 */
              "wubi: empty code notice");
        ArchtypeKey space = {sizeof(ArchtypeKey), ' ', 0, 0, 0};
        f = archtype_session_key(s, &space);
        check(f && f->handled && f->commit == NULL && f->preedit[0] == 0, "wubi: space clears empty code");
        archtype_session_free(s);
    }

    /* ABI 1.10：自定义词的编码整理 */
    {
        const char *code = archtype_custom_code("Ni H\xC7\x8Eo", 1);
        check(code && strcmp(code, "ni hao") == 0, "custom code: pinyin");
        code = archtype_custom_code("Mail", 0);
        check(code && strcmp(code, "mail") == 0, "custom code: letters");
        check(archtype_custom_code("Mail", 1) == NULL, "custom code: pinyin only");
        const char lex[] = "ni\t\xE4\xBD\xA0\t1\nhao\t\xE5\xA5\xBD\t1\n";
        code = archtype_guess_pinyin(lex, sizeof lex - 1, "\xE4\xBD\xA0\xE5\xA5\xBD");
        check(code && strcmp(code, "ni hao") == 0, "custom code: guess pinyin");
    }

    if (archtype_abi_version() / 1000 != ARCHTYPE_ABI_VERSION_MAJOR) {
        fprintf(stderr, "ABI 主版本不一致：库 %d，头文件 %d\n", archtype_abi_version(),
                ARCHTYPE_ABI_VERSION_MAJOR);
        return 1;
    }
    printf("abi=%d ", archtype_abi_version());
    printf("version=%s", archtype_version());
    putchar(10);
    archtype_engine_free(e);

    printf(failures ? "ABI SMOKE FAILED" : "ABI SMOKE OK");
    putchar(10);
    return failures ? 1 : 0;
}