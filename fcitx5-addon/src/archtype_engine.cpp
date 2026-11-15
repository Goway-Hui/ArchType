#include "archtype_engine.h"
#include "clipboard_copy.h"
#include "archtype_log.h"

#include <algorithm>
#include <chrono>
#include <cstddef>
#include <ctime>
#include <cstdint>
#include <cstdlib>
#include <exception>
#include <filesystem>
#include <fstream>
#include <memory>
#include <string>
#include <string_view>
#include <system_error>
#include <utility>

#include <fcitx-config/iniparser.h>
#include <fcitx/addonfactory.h>
#include <fcitx/addonmanager.h>
#include <fcitx/candidatelist.h>
#include <fcitx/globalconfig.h>
#include <fcitx/inputcontext.h>
#include <fcitx/inputcontextmanager.h>
#include <fcitx/inputpanel.h>
#include <fcitx/instance.h>
#include <fcitx/text.h>
#include <fcitx/userinterface.h>
#include <fcitx-utils/event.h>
#include <fcitx-utils/key.h>
#include <fcitx-utils/keysym.h>
#include <fcitx-utils/log.h>
#include <fcitx-utils/misc.h>
#include <fcitx-utils/utf8.h>
#include <fcitx-utils/textformatflags.h>

// fcitx5 自带的 clipboard 插件对外的函数（和它的 clipboard_public.h 一样，自 5.0 起没变过）。
// 抄在这里免得依赖 fcitx5-modules-dev；没装这个插件时 addon("clipboard") 是空的，不调用。
FCITX_ADDON_DECLARE_FUNCTION(Clipboard, clipboard, std::string(const fcitx::InputContext *ic));

namespace {

// 系统词库目录：CMake 按安装前缀传进来（Nix 等不装在 /usr 的发行版）
#ifndef ARCHTYPE_SYSTEM_DATA_DIR
#define ARCHTYPE_SYSTEM_DATA_DIR "/usr/share/archtype"
#endif
constexpr const char *kSystemDataDir = ARCHTYPE_SYSTEM_DATA_DIR;
/// 展开的候选面板：每行几个、一屏几行（照微信输入法：6 × 6）。
constexpr std::size_t kGridCols = 6;
constexpr std::size_t kGridRows = 6;
/// 候选总数上限（约 33 页 × 9 个）。
constexpr unsigned int kMaxCandidates = 1000;
/// 每上屏这么多次保存一次用户词库（另外失去焦点和退出时也保存）。
constexpr int kSaveEvery = 20;
/// 打字推荐的 emoji 插在第几个候选（从 0 数）：不挤掉前两个词。

bool envFlag(const char *name, bool fallback) {
    const char *value = std::getenv(name);
    if (!value || !*value) {
        return fallback;
    }
    return *value != '0';
}

constexpr const char *kConfigFile = "conf/archtype.conf";

/// 统计用的墙上时间（Unix 毫秒）：日界和 10 秒间隔都按它算。
int64_t statsNowMs() {
    return std::chrono::duration_cast<std::chrono::milliseconds>(
               std::chrono::system_clock::now().time_since_epoch())
        .count();
}

std::string userDataDir() {
    std::string base;
    if (const char *data = std::getenv("XDG_DATA_HOME"); data && *data) {
        base = data;
    } else if (const char *home = std::getenv("HOME"); home && *home) {
        base = std::string(home) + "/.local/share";
    } else {
        return {};
    }
    const std::string dir = base + "/archtype";
    std::error_code ec;
    if (!std::filesystem::exists(dir, ec)) {
        // 从旧品牌 WowType 升级：设置程序会把旧目录复制成 archtype；插件先照旧读，
        // 免得在设置程序跑之前，自定义词、输入习惯、统计看起来像丢了。
        const std::string legacy = base + "/wowtype";
        if (std::filesystem::exists(legacy, ec)) {
            return legacy;
        }
    }
    return dir;
}

/// 按顺序找第一个存在的文件：环境变量 > 用户数据目录 > 系统目录。
std::string findDataFile(const char *envName, const std::string &fileName) {
    if (const char *env = std::getenv(envName); env && *env) {
        return env;
    }
    std::error_code ec;
    const std::string user = userDataDir();
    if (!user.empty() && std::filesystem::exists(user + "/" + fileName, ec)) {
        return user + "/" + fileName;
    }
    const std::string system = std::string(kSystemDataDir) + "/" + fileName;
    if (std::filesystem::exists(system, ec)) {
        return system;
    }
    return {};
}

/// 去掉末尾一个 UTF-8 字符。
void popUtf8(std::string &text) {
    while (!text.empty()) {
        const auto byte = static_cast<unsigned char>(text.back());
        text.pop_back();
        if ((byte & 0xC0) != 0x80) {
            break;
        }
    }
}

constexpr uint64_t kVoicePollUsec = 50 * 1000;
constexpr std::size_t kVoiceBars = 6;

/// 音量（0–100）画成一排竖条，不足 kVoiceBars 个时左边补最低的条。
std::string voiceWave(const std::vector<int> &levels) {
    static const char *const kBlocks[] = {"▁", "▂", "▃", "▄", "▅", "▆", "▇", "█"};
    std::string out;
    for (std::size_t i = levels.size(); i < kVoiceBars; ++i) {
        out += kBlocks[0];
    }
    for (const int level : levels) {
        out += kBlocks[std::clamp(level, 0, 100) * 7 / 100];
    }
    return out;
}
constexpr uint64_t kVoiceErrorUsec = 1500 * 1000;
/// 「没有语音模型，去设置里下载」这句提示显示多久。
constexpr uint64_t kVoiceHintUsec = 4000 * 1000;
// 定时器精度。fcitx5 底下是 sd-event，传 0 等于默认的 250ms，50ms 的轮询实际一秒只有 4 次
constexpr uint64_t kTimerAccuracyUsec = 1000;

/// 把编辑区画进输入框。每一段都标上 DontCommit：失去焦点、应用重置输入框时，GTK / Qt 的输入模块
/// （XIM 这类由 fcitx5 代劳）会把编辑区里没标 DontCommit 的字直接上屏，那样文档里会留下
/// 「ni'hao」这种带分隔符的拼音、选了一半的「你好zhong'guo」、没译完的译文。打到一半的是放弃
/// （和 IBus 的默认一样），外壳在 reset / deactivate 里清掉会话。
void setClientPreedit(fcitx::InputPanel &panel, const fcitx::Text &text) {
    fcitx::Text shown;
    for (std::size_t i = 0; i < text.size(); ++i) {
        shown.append(text.stringAt(i), text.formatAt(i) | fcitx::TextFormatFlag::DontCommit);
    }
    shown.setCursor(text.cursor());
    panel.setClientPreedit(shown);
}

std::string userConfigDir() {
    if (const char *config = std::getenv("XDG_CONFIG_HOME"); config && *config) {
        return std::string(config) + "/archtype";
    }
    if (const char *home = std::getenv("HOME"); home && *home) {
        return std::string(home) + "/.config/archtype";
    }
    return {};
}

// 语音浮条：ArchType 的 GNOME 扩展注册这个名字，插件把语音状态发给它画。
// 信号 State(u 状态, s 文字, i 音量 0–100, i x, i y, i w, i h, b 相对窗口, d 缩放)，
// x/y/w/h 是光标矩形，和 kimpanel 的 SetSpotRect / SetRelativeSpotRectV2 同一个意思。
// 扩展画好了回一个 Ack(u 状态) 给插件；一次说话里等了 kOverlayAckUsec 还没有回执（扩展在这个
// GNOME 上画出错、或者是不会回执的旧版扩展），就改在候选框里提示，不会什么都不显示（#170）。
constexpr const char *kOverlayService = "io.github.gowayhui.ArchType.VoiceOverlay";
constexpr const char *kOverlayPath = "/io/github/gowayhui/ArchType";
constexpr const char *kOverlayInterface = "io.github.gowayhui.ArchType.Voice1";
constexpr uint32_t kOverlayHide = 0;
constexpr uint32_t kOverlayListening = 1;
constexpr uint32_t kOverlayRecognizing = 2;
constexpr uint32_t kOverlayDone = 3;
constexpr uint32_t kOverlayError = 4;
constexpr uint64_t kOverlayAckUsec = 800 * 1000;

/// 用户输入习惯（词的热度、词搭配）和用户词库放在同一个目录：history.tsv。
std::string historyPathFor(const std::string &userDict) {
    if (const char *env = std::getenv("ARCHTYPE_HISTORY"); env && *env) {
        return env;
    }
    if (userDict.empty()) {
        return {};
    }
    return (std::filesystem::path(userDict).parent_path() / "history.tsv").string();
}

/// 默认词库：用户数据目录或系统目录里的 lexicon.tsv（雾凇），都没有就用 demo 词典。
std::string defaultLexicon() {
    std::string lexicon = findDataFile("ARCHTYPE_LEXICON", "lexicon.tsv");
    if (lexicon.empty()) {
        lexicon = std::string(kSystemDataDir) + "/demo_lexicon.tsv";
    }
    return lexicon;
}

/// 实际用哪个词库：环境变量 ARCHTYPE_LEXICON > 配置里的 Lexicon（文件在才用）> 默认。
std::string resolveLexicon(const std::string &configured) {
    if (const char *env = std::getenv("ARCHTYPE_LEXICON"); env && *env) {
        return env;
    }
    if (!configured.empty()) {
        std::string path = configured;
        if (path.rfind("~/", 0) == 0) {
            if (const char *home = std::getenv("HOME"); home && *home) {
                path = std::string(home) + path.substr(1);
            }
        }
        std::error_code ec;
        if (std::filesystem::is_regular_file(path, ec)) {
            return path;
        }
        WLOG_W() << "ArchType: 配置里的词库 " << path << " 不在，用默认词库";
    }
    return defaultLexicon();
}

/// 文件的 (大小, 修改时间)，拿不到是 (0, 0)。
std::pair<std::uintmax_t, long long> fileStamp(const std::string &path) {
    std::error_code ec;
    const auto size = std::filesystem::file_size(path, ec);
    if (ec) {
        return {0, 0};
    }
    const auto time = std::filesystem::last_write_time(path, ec);
    if (ec) {
        return {0, 0};
    }
    return {size, static_cast<long long>(time.time_since_epoch().count())};
}

/// 用这个词库建核心；语言模型坏了（版本不对、文件不完整）不该让输入法整个用不了，
/// 不带它再试一次（这时 lm 清空）。
ArchtypeEngineHandle *openCore(const std::string &lexicon, std::string &lm) {
    ArchtypeEngineHandle *core =
        archtype_engine_new(lexicon.c_str(), lm.empty() ? nullptr : lm.c_str());
    if (!core && !lm.empty()) {
        WLOG_W() << "ArchType: 语言模型载入失败，不用它，文件=" << lm << "，错误="
                     << (archtype_last_error() ? archtype_last_error() : "unknown");
        lm.clear();
        core = archtype_engine_new(lexicon.c_str(), nullptr);
    }
    return core;
}

/// 在后台线程里做的事：载入词库、语言模型、各种开关和用户词。
ArchtypeEngineHandle *loadCore(const std::string &userDict, std::string lexicon) {
    if (lexicon.ends_with("/demo_lexicon.tsv")) {
        WLOG_W() << "ArchType: 没找到 lexicon.tsv，用 demo 词典（只有几十个词）";
    }
    // 词级语言模型：archtype-lmbuild 生成的 words.lm；没有就找旧的 bigram.tsv
    std::string lm = findDataFile("ARCHTYPE_LM", "words.lm");
    if (lm.empty()) {
        lm = findDataFile("ARCHTYPE_LM", "bigram.tsv");
    }
    const std::string lmFile = lm;
    const auto started = std::chrono::steady_clock::now();
    ArchtypeEngineHandle *core = openCore(lexicon, lm);
    if (!core) {
        // 选的词库（比如下载的万象）坏了：退回默认词库，别让输入法用不了
        if (const std::string fallback = defaultLexicon(); fallback != lexicon) {
            WLOG_W() << "ArchType: 词库 " << lexicon << " 载入失败（"
                         << (archtype_last_error() ? archtype_last_error() : "unknown")
                         << "），改用默认词库 " << fallback;
            lexicon = fallback;
            lm = lmFile;
            core = openCore(lexicon, lm);
        }
    }
    if (!core) {
        WLOG_E() << "ArchType: 初始化 Rust 核心失败，词典=" << lexicon
                      << "，错误="
                      << (archtype_last_error() ? archtype_last_error() : "unknown");
        return nullptr;
    }

    const std::string english = findDataFile("ARCHTYPE_ENGLISH", "english.txt");
    if (!english.empty()) {
        const int loaded = archtype_engine_load_english(core, english.c_str());
        if (loaded > 0) {
            WLOG_I() << "ArchType: 载入英文词 " << loaded << " 个";
        }
    }
    // 微信输入法的字级神经语言模型（不随 ArchType 分发，本机有才用）
    if (const std::string nnlm = findDataFile("ARCHTYPE_NNLM", "wetype/nlrgpt2_nnlm.bin");
        !nnlm.empty()) {
        const int loaded = archtype_engine_load_nnlm(core, nnlm.c_str());
        if (loaded > 0) {
            WLOG_I() << "ArchType: 载入神经语言模型（" << loaded << " 字）";
        } else {
            WLOG_W() << "ArchType: 神经语言模型载入失败 " << nnlm << "："
                         << (archtype_last_error() ? archtype_last_error() : "unknown");
        }
    }
    // 用户自己训的领域词级模型（可选），和 words.lm 混着用
    if (const std::string domain = findDataFile("ARCHTYPE_DOMAIN_LM", "words.domain.lm");
        !domain.empty()) {
        if (archtype_engine_set_domain_lm(core, domain.c_str()) == 0) {
            WLOG_I() << "ArchType: 载入领域词级模型 " << domain;
        } else {
            WLOG_W() << "ArchType: 领域词级模型载入失败 " << domain << "："
                         << (archtype_last_error() ? archtype_last_error() : "unknown");
        }
    }
    if (!userDict.empty()) {
        const int loaded = archtype_engine_load_user_dict(core, userDict.c_str());
        if (loaded > 0) {
            WLOG_I() << "ArchType: 载入用户词 " << loaded << " 条";
        }
    }
    if (const std::string history = historyPathFor(userDict);
        !history.empty() && std::filesystem::exists(history)) {
        const int loaded = archtype_engine_load_history(core, history.c_str());
        if (loaded >= 0) {
            WLOG_I() << "ArchType: 载入输入习惯 " << loaded << " 条";
        } else {
            WLOG_W() << "ArchType: 输入习惯载入失败 " << history << "："
                         << (archtype_last_error() ? archtype_last_error() : "unknown");
        }
    }
    const auto ms = std::chrono::duration_cast<std::chrono::milliseconds>(
                        std::chrono::steady_clock::now() - started)
                        .count();
    WLOG_I() << "ArchType: 词典 " << lexicon << " 载入完成，用时 " << ms << " ms"
                 << (lm.empty() ? "（没有语言模型）" : "，语言模型 " + lm);
    return core;
}

/// 候选窗里显示的样子：自定义词里的换行、制表符画成 ⏎、⇥，不把候选窗撑乱。
std::string shownText(const std::string &text) {
    if (text.find_first_of("\n\t") == std::string::npos) {
        return text;
    }
    std::string out;
    for (const char c : text) {
        if (c == '\n') {
            out += "⏎";
        } else if (c == '\t') {
            out += "⇥";
        } else {
            out.push_back(c);
        }
    }
    return out;
}

class ArchtypeCandidateWord final : public fcitx::CandidateWord {
public:
    /// shown 是 shownCandidate 画好的样子。
    ArchtypeCandidateWord(ArchtypeEngine *engine, ArchtypeState *state,
                         std::size_t index, fcitx::Text shown)
        : engine_(engine), state_(state), index_(index) {
        setText(std::move(shown));
    }

    void select(fcitx::InputContext *inputContext) const override {
        engine_->commitCandidate(inputContext, state_, index_);
    }

private:
    ArchtypeEngine *engine_;
    ArchtypeState *state_;
    std::size_t index_;
};

/// 展开面板的一行（一行里有好几个词）。
class ArchtypeGridRow final : public fcitx::CandidateWord {
public:
    ArchtypeGridRow(ArchtypeEngine *engine, ArchtypeState *state, std::size_t row, fcitx::Text text)
        : engine_(engine), state_(state), row_(row) {
        setText(std::move(text));
    }

    void select(fcitx::InputContext *inputContext) const override {
        engine_->selectGridRow(inputContext, state_, row_);
    }

private:
    ArchtypeEngine *engine_;
    ArchtypeState *state_;
    std::size_t row_;
};

/// 工具栏列表里的一项（菜单、剪贴板历史、常用语）。
class ArchtypeToolWord final : public fcitx::CandidateWord {
public:
    ArchtypeToolWord(ArchtypeEngine *engine, ArchtypeState *state, std::size_t index,
                    const std::string &text)
        : engine_(engine), state_(state), index_(index) {
        setText(fcitx::Text(text));
    }

    void select(fcitx::InputContext *inputContext) const override {
        engine_->choosePanelItem(inputContext, state_, index_);
    }

private:
    ArchtypeEngine *engine_;
    ArchtypeState *state_;
    std::size_t index_;
};

/// 显示宽度（半角为 1、其他为 2），用来把展开面板的各列对齐。
/// emoji 序列（变体选择符、肤色、零宽连接、旗帜）按一个宽字符算。
std::size_t displayWidth(const std::string &text) {
    std::size_t width = 0;
    bool joinNext = false;
    bool pendingFlag = false;
    for (std::size_t i = 0; i < text.size();) {
        const unsigned char c = static_cast<unsigned char>(text[i]);
        const std::size_t len = c < 0x80 ? 1 : c < 0xE0 ? 2 : c < 0xF0 ? 3 : 4;
        char32_t cp = len == 1 ? c : c & (0xFF >> (len + 1));
        for (std::size_t k = 1; k < len && i + k < text.size(); ++k) {
            cp = (cp << 6) | (static_cast<unsigned char>(text[i + k]) & 0x3F);
        }
        i += len;
        if (cp == 0x200D) {
            joinNext = true;
            continue;
        }
        const bool zeroWidth = cp == 0xFE0F || cp == 0xFE0E || cp == 0x20E3 ||
                               (cp >= 0x1F3FB && cp <= 0x1F3FF) ||
                               (cp >= 0xE0020 && cp <= 0xE007F);
        if (zeroWidth || joinNext) {
            joinNext = false;
            continue;
        }
        if (cp >= 0x1F1E6 && cp <= 0x1F1FF) { // 两个地区指示符拼成一面旗
            if (pendingFlag) {
                pendingFlag = false;
                continue;
            }
            pendingFlag = true;
            width += 2;
            continue;
        }
        pendingFlag = false;
        width += cp < 0x80 ? 1 : 2;
    }
    return width;
}

/// 用全角空格（宽 2）和半角空格（宽 1）补到 width。
void padTo(std::string &out, std::size_t have, std::size_t width) {
    while (have + 2 <= width) {
        out += "\u3000";
        have += 2;
    }
    if (have < width) {
        out += "\u2002";
    }
}

} // namespace

struct ArchtypeCandidate {
    std::string text;
    std::string pinyin;
    /// 用掉了剩余输入的多少字节；小于剩余长度说明是句首词候选。
    std::size_t consumed = 0;
    /// emoji 候选：选中时上屏 commit，记进最近用过的 emoji，不学习选词。
    bool emoji = false;
    std::string commit;
    /// emoji 本身（打字推荐的候选显示的是「开心😊」，这里是「😊」）。
    std::string emojiText;
    /// 日期时间、计算结果这类：选中时原样上屏，不学习选词。
    bool plain = false;
    /// 画在字后面的编码提示（五笔还要打的码、z 临时拼音反查的五笔码），不上屏。
    std::string comment;
};

namespace {

/// 候选框里一个候选的样子：字（换行画成 ⏎），后面空一格是斜体的编码提示，和字分得开。
/// fcitx5 5.1.9 以前候选没有注释位，经典界面也只认粗体、斜体、下划线、删除线，画不了浅色，
/// 斜体是各个版本都一样的那个。
fcitx::Text shownCandidate(const ArchtypeCandidate &candidate) {
    fcitx::Text out(shownText(candidate.text));
    if (!candidate.comment.empty()) {
        out.append(" " + candidate.comment, fcitx::TextFormatFlag::Italic);
    }
    return out;
}

} // namespace

/// 每个输入上下文独立保存的缓冲区和候选。
class ArchtypeState final : public fcitx::InputContextProperty {
public:
    ArchtypeState() : session_(archtype_session_new(nullptr, nullptr)) {}
    ~ArchtypeState() override { archtype_session_free(session_); }
    ArchtypeState(const ArchtypeState &) = delete;
    ArchtypeState &operator=(const ArchtypeState &) = delete;

    /// 核心的输入会话：编辑区的原始输入和已选词栈在这里（见 docs/PORTING.md）。
    /// 表情面板开着时，原始输入就是搜索词。
    ArchtypeSession *session_;
    std::vector<ArchtypeCandidate> candidates_;
    /// 展开的面板（普通候选下键展开，或表情面板）；gridCursor_ 是光标所在候选的全局下标。
    /// 普通候选的这些（连同下面的页码、光标）是会话的候选框状态的拷贝，见 syncView。
    bool expanded_ = false;
    std::size_t gridCursor_ = 0;
    /// 普通候选列表的页码和光标（全局下标，可以不在这一页上）；工具栏也用 panelCursor_。
    std::size_t panelPage_ = 0;
    std::size_t panelCursor_ = 0;
    /// candidates_ 抄的是会话第几次重算的候选（ArchtypeSessionView.generation）。
    uint64_t candGeneration_ = UINT64_MAX;
    /// 开着的工具栏 / 表情面板（ARCHTYPE_TOOL_*），会话管。
    int panel() const { return archtype_session_open_panel(session_); }

    /// 全部原始输入（包括已经选过词的部分）。
    std::string input() const { return archtype_session_input(session_); }
    /// 还没选词的那部分输入。
    std::string remaining() const { return archtype_session_remaining(session_); }
    std::string selectedText() const { return archtype_session_selected_text(session_); }
    bool hasSelections() const { return !selectedText().empty(); }
    bool composing() const { return *archtype_session_input(session_) != '\0'; }
    /// 交给会话处理一个按下的键（keysym 为 0 时只有动作）。先用 ArchtypeEngine::sessionKey
    /// 同步引擎和设置。返回的 frame 归会话，下一次调会话前有效。
    /// prev_char：应用里光标前一个字（读不到给 0）；hints：ARCHTYPE_KEY_HINT_*。
    const ArchtypeFrame *sessionKey(uint32_t keysym, uint32_t modifiers, uint32_t action,
                                   uint32_t prevChar = 0, uint32_t hints = 0,
                                   uint64_t timeMs = 0) {
        ArchtypeKey raw{};
        raw.time_ms = timeMs;
        raw.size = sizeof(raw);
        raw.keysym = keysym;
        raw.modifiers = modifiers;
        raw.action = action;
        raw.prev_char = prevChar;
        raw.hints = hints;
        return archtype_session_key(session_, &raw);
    }
    void clear() {
        archtype_session_reset(session_, 0);
        forgetCandidates();
    }

    /// 只丢掉外壳抄的候选和候选框状态，会话的编辑区不动。
    void forgetCandidates() {
        candidates_.clear();
        candGeneration_ = UINT64_MAX;
        expanded_ = false;
        gridCursor_ = 0;
        panelPage_ = 0;
        panelCursor_ = 0;
    }
};

namespace {

/// 普通候选的列表：翻页、移动光标交给会话，再照会话的页码和光标摆好自己。
/// 键盘上的翻页键由会话直接处理；这里接的是鼠标点翻页按钮、滚轮，和直接调候选列表的。
class ArchtypeSessionList final : public fcitx::CommonCandidateList {
public:
    explicit ArchtypeSessionList(ArchtypeState *state) : state_(state) {}

    void prev() override { act(ARCHTYPE_KEY_ACTION_PREV_PAGE); }
    void next() override { act(ARCHTYPE_KEY_ACTION_NEXT_PAGE); }
    void prevCandidate() override { act(ARCHTYPE_KEY_ACTION_PREV_CANDIDATE); }
    void nextCandidate() override { act(ARCHTYPE_KEY_ACTION_NEXT_CANDIDATE); }
    void setPage(int page) override {
        if (mirroring_) {
            // mirror 里调的 CommonCandidateList::next() 会走到这里：只摆自己，不再翻会话
            CommonCandidateList::setPage(page);
            return;
        }
        for (int guard = 0; guard < 1000 && page != currentPage(); ++guard) {
            const int before = currentPage();
            act(page < before ? ARCHTYPE_KEY_ACTION_PREV_PAGE : ARCHTYPE_KEY_ACTION_NEXT_PAGE);
            if (currentPage() == before) {
                break;
            }
        }
    }

    /// 照会话的页码和光标摆好（页码往后翻一页时用 next()，fcitx5 记得「用过下一页」）。
    void mirror(std::size_t page, std::size_t cursor) {
        mirroring_ = true;
        const int target = static_cast<int>(page);
        if (target != currentPage() && target >= 0 && target < totalPages()) {
            if (target == currentPage() + 1) {
                CommonCandidateList::next();
            } else {
                CommonCandidateList::setPage(target);
            }
        }
        if (static_cast<int>(cursor) < totalSize()) {
            setGlobalCursorIndex(static_cast<int>(cursor));
        }
        mirroring_ = false;
    }

private:
    void act(uint32_t action) {
        state_->sessionKey(0, 0, action);
        ArchtypeSessionView view{};
        view.size = sizeof(view);
        if (archtype_session_view(state_->session_, &view) != 0) {
            return;
        }
        state_->panelPage_ = view.page;
        state_->panelCursor_ = view.cursor;
        mirror(view.page, view.cursor);
    }

    ArchtypeState *state_;
    bool mirroring_ = false;
};

} // namespace

ArchtypeEngine::ArchtypeEngine(fcitx::Instance *instance)
    : instance_(instance),
      core_(nullptr),
      factory_([](fcitx::InputContext &) { return new ArchtypeState(); }) {
    instance_->inputContextManager().registerProperty("archtype", &factory_);
    WLOG_I() << "ArchType " << ARCHTYPE_ADDON_VERSION << " 启动（fcitx5 " << fcitx::Instance::version()
             << "）";
    fcitx::RawConfig raw;
    fcitx::readAsIni(raw, kConfigFile);
    loadArchtypeConfig(config_, raw);
    // 全量词库载入要几百毫秒，放后台，别卡住 fcitx 启动
    lexicon_ = wantedLexicon_ = resolveLexicon(*config_.lexicon);
    lexiconStamp_ = fileStamp(lexicon_);
    loading_ = std::async(std::launch::async, loadCore, userDictPath(), lexicon_);
    applyConfig();
    setupVoiceOverlay();
    setupControl();
    const std::string statsDir = userDataDir();
    if (!statsDir.empty()) {
        const std::string statsPath = statsDir + "/stats.json";
        stats_ = archtype_stats_new(statsPath.c_str());
        if (!stats_) {
            WLOG_W() << "ArchType: 建不了打字统计 " << statsPath << "："
                     << (archtype_last_error() ? archtype_last_error() : "unknown");
        }
    }
}

namespace {
constexpr char kControlService[] = "org.archtype.Input";
constexpr char kControlPath[] = "/org/archtype/Input";
constexpr char kControlInterface[] = "org.archtype.Input1";
} // namespace

std::string ArchtypeControl::mode() { return engine_->modeName(); }

std::string ArchtypeControl::setMode(const std::string &mode) {
    if (!engine_->setModeByName(mode)) {
        throw fcitx::dbus::MethodCallError("org.freedesktop.DBus.Error.InvalidArgs",
                                           ("不认识的模式或现在切不了：" + mode +
                                            "（可以是 zh、en、translate、toggle）")
                                               .c_str());
    }
    return engine_->modeName();
}

void ArchtypeEngine::setupControl() {
    try {
        auto bus = std::make_unique<fcitx::dbus::Bus>(fcitx::dbus::BusType::Session);
        if (!bus->isOpen()) {
            return;
        }
        bus->attachEventLoop(&instance_->eventLoop());
        auto control = std::make_unique<ArchtypeControl>(this);
        if (!bus->addObjectVTable(kControlPath, kControlInterface, *control) ||
            !bus->requestName(kControlService,
                              fcitx::Flags<fcitx::dbus::RequestNameFlag>{
                                  fcitx::dbus::RequestNameFlag::ReplaceExisting,
                                  fcitx::dbus::RequestNameFlag::AllowReplacement})) {
            WLOG_W() << "ArchType: 登记 " << kControlService << " 失败，外部命令切不了模式";
            return;
        }
        controlBus_ = std::move(bus);
        control_ = std::move(control);
    } catch (const std::exception &e) {
        WLOG_I() << "ArchType: 连不上会话总线，外部命令切不了模式：" << e.what();
    }
}

std::string ArchtypeEngine::modeName() const {
    return translateMode_ ? "translate" : asciiMode_ ? "en" : "zh";
}

bool ArchtypeEngine::setModeByName(const std::string &mode) {
    bool ascii = false;
    bool translate = false;
    if (mode == "en") {
        ascii = true;
    } else if (mode == "translate") {
        if (!translateAvailable()) {
            return false;
        }
        translate = true;
    } else if (mode == "toggle") {
        ascii = !asciiMode_ && !translateMode_;
    } else if (mode != "zh") {
        return false;
    }
    if (ascii == asciiMode_ && translate == translateMode_) {
        return true;
    }
    // 切的是最近用过的输入框（vim 所在的终端）；它没在用 ArchType 时只改模式
    fcitx::InputContext *ic = instance_->mostRecentInputContext();
    ArchtypeState *state = ic ? ic->propertyFor(&factory_) : nullptr;
    switchMode(ic, state, ascii, translate, false);
    return true;
}

void ArchtypeEngine::setupVoiceOverlay() {
    // ARCHTYPE_VOICE_OVERLAY=0：不用 GNOME 扩展的浮条，一直用候选框里的文字提示
    if (!envFlag("ARCHTYPE_VOICE_OVERLAY", true)) {
        return;
    }
    try {
        auto bus = std::make_unique<fcitx::dbus::Bus>(fcitx::dbus::BusType::Session);
        if (!bus->isOpen()) {
            return;
        }
        bus->attachEventLoop(&instance_->eventLoop());
        overlayBus_ = std::move(bus);
        overlayWatcher_ = std::make_unique<fcitx::dbus::ServiceWatcher>(*overlayBus_);
        overlayWatch_ = overlayWatcher_->watchService(
            kOverlayService,
            [this](const std::string &, const std::string &, const std::string &owner) {
                if (owner != overlayOwner_) {
                    overlayOwner_ = owner;
                    // 换了一个扩展实例（重新登录、扩展重载）：再给浮条一次机会
                    overlayBroken_ = false;
                    overlayAcked_ = false;
                    overlaySentAt_ = 0;
                    WLOG_I() << "ArchType: 语音浮条" << (owner.empty() ? "不在了" : "就绪");
                }
            });
        overlayAckSlot_ = overlayBus_->addMatch(
            fcitx::dbus::MatchRule(kOverlayService, kOverlayPath, kOverlayInterface, "Ack"),
            [this](fcitx::dbus::Message &msg) {
                uint32_t state = kOverlayHide;
                msg >> state;
                // 只算这次说话里、显示出来的状态的回执：上一次收起时的回执可能这时才到
                if (msg.sender() == overlayOwner_ && overlaySentAt_ && state != kOverlayHide &&
                    state != kOverlayDone) {
                    overlayAcked_ = true;
                }
                return true;
            });
    } catch (const std::exception &e) {
        // 没有会话总线（比如测试里）：用文字提示
        WLOG_I() << "ArchType: 连不上会话总线，语音用文字提示：" << e.what();
        overlayAckSlot_.reset();
        overlayWatch_.reset();
        overlayWatcher_.reset();
        overlayBus_.reset();
    }
}

void ArchtypeEngine::sendVoiceOverlay(fcitx::InputContext *ic, uint32_t state,
                                     const std::string &label, int level) {
    if (!overlayBus_ || overlayOwner_.empty()) {
        return;
    }
    int32_t x = 0, y = 0, w = 0, h = 0;
    bool relative = false;
    double scale = 1.0;
    if (ic) {
        const fcitx::Rect &rect = ic->cursorRect();
        x = rect.left();
        y = rect.top();
        w = rect.width();
        h = rect.height();
        relative = ic->capabilityFlags().test(fcitx::CapabilityFlag::RelativeRect);
        scale = ic->scaleFactor();
    }
    auto msg = overlayBus_->createSignal(kOverlayPath, kOverlayInterface, "State");
    msg.setDestination(overlayOwner_);
    msg << state << label << static_cast<int32_t>(std::clamp(level, 0, 100)) << x << y << w << h
        << relative << scale;
    msg.send();
    if (state != kOverlayHide && state != kOverlayDone && !overlaySentAt_) {
        overlaySentAt_ = fcitx::now(CLOCK_MONOTONIC);
    }
}

void ArchtypeEngine::checkOverlayAck(fcitx::InputContext *ic) {
    if (!overlaySentAt_ || overlayAcked_ || overlayBroken_ ||
        fcitx::now(CLOCK_MONOTONIC) - overlaySentAt_ < kOverlayAckUsec) {
        return;
    }
    overlayBroken_ = true;
    WLOG_W() << "ArchType: 语音浮条没有回应（GNOME 扩展画的时候出错了？），改在候选框里提示；"
                "重新登录后会再试浮条";
    // 正留着的错误提示马上换到候选框里；录音、识别中的提示下一轮就按候选框画
    if (voiceErrorUntil_ && !voiceErrorText_.empty()) {
        showVoice(ic, "🎙 " + voiceErrorText_, {});
    }
}

void ArchtypeEngine::setConfig(const fcitx::RawConfig &config) {
    config_.load(config, true);
    fcitx::safeSaveAsIni(config_, kConfigFile);
    applyConfig();
}

void ArchtypeEngine::reloadConfig() {
    // 文件里没写的项回到默认值（设置界面删掉一项就是想恢复默认）
    config_ = ArchtypeConfig();
    fcitx::RawConfig raw;
    fcitx::readAsIni(raw, kConfigFile);
    loadArchtypeConfig(config_, raw);
    applyConfig();
    wantedLexicon_ = resolveLexicon(*config_.lexicon);
    maybeSwitchLexicon();
}

void ArchtypeEngine::maybeSwitchLexicon() {
    if (wantedLexicon_ == lexicon_ && fileStamp(lexicon_) == lexiconStamp_) {
        return;
    }
    if (loading_.valid()) {
        if (loading_.wait_for(std::chrono::seconds(0)) != std::future_status::ready) {
            return; // 还在载入上一个：coreReady() 载完时再来
        }
        core_ = loading_.get();
    }
    if (core_) {
        maybeSaveUserDict(true);
    }
    // 候选、编辑区都是旧词库算出来的，先清掉
    instance_->inputContextManager().foreach([this](fcitx::InputContext *ic) {
        auto *state = ic->propertyFor(&factory_);
        state->clear();
        clearTranslation(state);
        // 会话借用引擎：先让它放开，再释放（ABI.md：引擎要比会话活得久）
        archtype_session_set_engine(state->session_, nullptr);
        ic->inputPanel().reset();
        ic->updatePreedit();
        ic->updateUserInterface(fcitx::UserInterfaceComponent::InputPanel);
        return true;
    });
    if (core_) {
        archtype_engine_free(core_);
        core_ = nullptr;
        // 简繁表、五笔码表是载进这个引擎的，新引擎载好后 applyCoreSettings 再载
        conversionLoaded_ = false;
        wubiLoaded_ = false;
    }
    if (lexicon_ == wantedLexicon_) {
        WLOG_I() << "ArchType: 词库文件换了，重新载入 " << lexicon_;
    } else {
        WLOG_I() << "ArchType: 换词库 " << lexicon_ << " -> " << wantedLexicon_;
    }
    lexicon_ = wantedLexicon_;
    lexiconStamp_ = fileStamp(lexicon_);
    loading_ = std::async(std::launch::async, loadCore, userDictPath(), lexicon_);
}

void ArchtypeEngine::applyConfig() {
    const auto envOr = [](const char *name, bool configured) {
        return envFlag(name, configured);
    };
    horizontal_ = !envOr("ARCHTYPE_VERTICAL", *config_.layout == ArchtypeLayout::Vertical);
    arrowKeys_ = *config_.arrowKeys;
    pageSize_ = static_cast<std::size_t>(std::clamp(*config_.pageSize, 3, 9));
    pageMinus_ = *config_.pageKeysMinus;
    pageBracket_ = *config_.pageKeysBracket;
    pageComma_ = *config_.pageKeysComma;
    pageTab_ = *config_.pageKeysTab;
    if (const char *env = std::getenv("ARCHTYPE_PAGE_KEYS"); env && *env) {
        // 逗号分隔：minus,bracket,comma,tab
        const std::string keys = std::string(",") + env + ",";
        pageMinus_ = keys.find(",minus,") != std::string::npos;
        pageBracket_ = keys.find(",bracket,") != std::string::npos;
        pageComma_ = keys.find(",comma,") != std::string::npos;
        pageTab_ = keys.find(",tab,") != std::string::npos;
    }
    fuzzy_ = envOr("ARCHTYPE_FUZZY", *config_.fuzzy);
    // 位掩码，和核心的 syllable::fuzzy 一致
    fuzzyGroups_ = (*config_.fuzzyZh ? 1u : 0u) | (*config_.fuzzyCh ? 2u : 0u) |
                   (*config_.fuzzySh ? 4u : 0u) | (*config_.fuzzyNL ? 8u : 0u) |
                   (*config_.fuzzyFH ? 16u : 0u) | (*config_.fuzzyRL ? 32u : 0u) |
                   (*config_.fuzzyAn ? 64u : 0u) | (*config_.fuzzyEn ? 128u : 0u) |
                   (*config_.fuzzyIn ? 256u : 0u) | (*config_.fuzzyIan ? 512u : 0u) |
                   (*config_.fuzzyUan ? 1024u : 0u) | (*config_.fuzzyUn ? 2048u : 0u) |
                   (*config_.fuzzyAnAi ? 4096u : 0u) | (*config_.fuzzyHuangWang ? 8192u : 0u) |
                   (*config_.fuzzyUnIong ? 16384u : 0u) | (*config_.fuzzyEngOng ? 32768u : 0u);
    if (const char *env = std::getenv("ARCHTYPE_FUZZY_GROUPS"); env && *env) {
        fuzzyGroups_ = static_cast<unsigned>(std::strtoul(env, nullptr, 0));
    }
    selectKeys_ = *config_.selectKeys;
    if (const char *env = std::getenv("ARCHTYPE_SELECT_KEYS"); env && *env) {
        const std::string name = env;
        selectKeys_ = name == "semicolon"   ? ArchtypeSelectKeys::Semicolon
                      : name == "quote"     ? ArchtypeSelectKeys::SemicolonQuote
                                            : ArchtypeSelectKeys::Off;
    }
    pickChar_ = envOr("ARCHTYPE_PICK_CHAR", *config_.pickChar);
    dateTime_ = envOr("ARCHTYPE_DATETIME", *config_.dateTime);
    calculator_ = envOr("ARCHTYPE_CALC", *config_.calculator);
    toolbar_ = envOr("ARCHTYPE_TOOLBAR", *config_.toolbar);
    clipboardHistory_ = envOr("ARCHTYPE_CLIPBOARD_HISTORY", *config_.clipboardHistory);
    shuangpinShowKeys_ = *config_.shuangpinShowKeys;
    if (!clipboardHistory_) {
        archtype_clipboard_clear();
    }
    autoSpaceFlags_ = *config_.autoSpace ? (*config_.autoSpaceLetters ? 1 : 0) |
                                               (*config_.autoSpaceDigits ? 2 : 0)
                                         : 0;
    if (const char *env = std::getenv("ARCHTYPE_AUTOSPACE"); env && *env) {
        autoSpaceFlags_ = static_cast<int>(std::strtol(env, nullptr, 0)) & 3;
    }
    typo_ = envOr("ARCHTYPE_TYPO", *config_.typo);
    rareExt_ = envOr("ARCHTYPE_RARE_EXT", *config_.rareExt);
    shuangpin_ = envOr("ARCHTYPE_SHUANGPIN", *config_.inputScheme == ArchtypeInputScheme::Shuangpin);
    shuangpinScheme_ = ArchtypeShuangpinSchemeToString(*config_.shuangpinScheme);
    if (const char *env = std::getenv("ARCHTYPE_SHUANGPIN_SCHEME"); env && *env) {
        // 环境变量设了方案就当作打开双拼（方便测试）
        shuangpinScheme_ = env;
        shuangpin_ = true;
    }
    // 双拼和五笔只能选一个（环境变量打开了双拼时就不是五笔）
    wubi_ = !shuangpin_ && *config_.inputScheme == ArchtypeInputScheme::Wubi86;
    shiftToggle_ = *config_.shiftToggle;
    if (const char *env = std::getenv("ARCHTYPE_SHIFT_TOGGLE"); env && *env) {
        const std::string name = env;
        shiftToggle_ = name == "left"    ? ArchtypeShiftToggle::Left
                       : name == "right" ? ArchtypeShiftToggle::Right
                       : name == "off" || name == "0" ? ArchtypeShiftToggle::Off
                                                      : ArchtypeShiftToggle::Both;
    }
    toggleKey_ = *config_.toggleKey;
    translateModeKey_ = *config_.translateModeKey;
    if (!canToggleMode()) { // 切不回中文了，回到中文
        asciiMode_ = false;
        translateMode_ = false;
    }
    initialMode_ = *config_.initialMode;
    punctStyle_ = *config_.punctStyle;
    if (const char *env = std::getenv("ARCHTYPE_PUNCT_STYLE"); env && *env) {
        const std::string name = env;
        punctStyle_ = name == "half" || name == "halfwidth" ? ArchtypePunctStyle::Halfwidth
                                                            : ArchtypePunctStyle::Fullwidth;
    }
    traditional_ = envOr("ARCHTYPE_TRADITIONAL", *config_.traditional);
    emojiSuggest_ = envOr("ARCHTYPE_EMOJI", *config_.emojiSuggest);
    emojiKey_ = *config_.emojiKey;

    // 语音热键：ARCHTYPE_VOICE_KEY（fcitx 键名，如 Alt_R、Control_R、F8）优先，none 关掉
    fcitx::Key key = *config_.voice ? *config_.voiceKey : fcitx::Key();
    if (const char *env = std::getenv("ARCHTYPE_VOICE_KEY"); env && *env) {
        const std::string name = env;
        key = name == "none" || name == "0" ? fcitx::Key() : fcitx::Key(name);
        if (key.sym() == FcitxKey_None && name != "none" && name != "0") {
            WLOG_W() << "ArchType: 不认识语音热键 " << name << "，改用 Alt_R";
            key = fcitx::Key(FcitxKey_Alt_R);
        }
    }
    if (key.sym() != voiceKey_.sym() || key.states() != voiceKey_.states()) {
        if (voiceActive()) {
            cancelVoice();
        }
        voiceKeyDown_ = false;
    }
    voiceKey_ = key;
    voiceCopy_ = envOr("ARCHTYPE_VOICE_COPY", *config_.voiceCopy);
    if (voiceKey_.sym() != FcitxKey_None && !voice_) {
        // 只建句柄：不开麦克风、不读模型，第一次按热键时才读。模型从来不在这里下载，
        // 没下载时按热键只提示去「ArchType 设置 → 语音」里下载
        const char *dir = std::getenv("ARCHTYPE_VOICE_MODEL_DIR");
        voice_ = archtype_voice_new(dir);
    }
    // 互译：环境变量 ARCHTYPE_TRANSLATE_URL / _MODEL / _TARGET / _KEY_FILE 覆盖（测试用）
    translateEnabled_ = envOr("ARCHTYPE_TRANSLATE", *config_.translate);
    std::string translateUrl = *config_.translateBaseUrl;
    std::string translateModel = *config_.translateModel;
    std::string translateTarget = ArchtypeTranslateTargetToString(*config_.translateTarget);
    if (const char *env = std::getenv("ARCHTYPE_TRANSLATE_URL"); env && *env) {
        translateUrl = env;
    }
    if (const char *env = std::getenv("ARCHTYPE_TRANSLATE_MODEL"); env && *env) {
        translateModel = env;
    }
    if (const char *env = std::getenv("ARCHTYPE_TRANSLATE_TARGET"); env && *env) {
        translateTarget = env;
    }
    if (translateEnabled_ && !translator_) {
        translator_ = archtype_translate_new();
    }
    if (translator_) {
        archtype_translate_configure(translator_, translateUrl.c_str(), translateModel.c_str(),
                                    translateTarget.c_str(), *config_.translateFormal ? 1 : 0,
                                    translateKeyFile().c_str());
        std::string localModel;
        if (*config_.translateLocal) {
            localModel = *config_.translateLocalModel;
            if (localModel.empty()) {
                localModel = userDataDir() + "/models/HY-MT1.5-1.8B-Q4_K_M.gguf";
            }
            translateModel = "本地 HY-MT";
        }
        // 没选本地模型时传空串：不起进程、不占端口（已经起了的马上停掉）
        archtype_translate_configure_local(
            translator_, localModel.c_str(), (*config_.translateLlamaServer).c_str(),
            static_cast<unsigned>(*config_.translateIdleMinutes) * 60u);
    }
    if (translateMode_ && !translateAvailable()) {
        translateMode_ = false;
    }

    WLOG_I() << "ArchType: 设置 " << (horizontal_ ? "横排" : "竖排")
             << (arrowKeys_ == ArchtypeArrowKeys::Caret ? " 左右键移光标" : "") << " 每页" << pageSize_
             << " 模糊音=" << fuzzy_ << "(" << fuzzyGroups_ << ")" << " 纠错=" << typo_ << " 扩展区生僻字=" << rareExt_ << " 双拼=" << shuangpin_
             << "(" << shuangpinScheme_ << ")" << " 五笔=" << wubi_ << " Shift切换=" << static_cast<int>(shiftToggle_)
             << " 繁体=" << traditional_ << " 语音键=" << voiceKey_.toString()
             << " emoji推荐=" << emojiSuggest_ << " 表情键=" << emojiKey_.toString()
             << " 中英键=" << toggleKey_.toString() << " 互译键=" << translateModeKey_.toString()
             << " 选词键=" << static_cast<int>(selectKeys_) << " 日期时间=" << dateTime_
             << " 计算器=" << calculator_ << " 工具栏=" << toolbar_
             << " 剪贴板历史=" << clipboardHistory_ << " 互译=" << translateEnabled_ << "("
             << (translateAvailable() ? translateModel + " → " + translateTarget : "未配置")
             << ")"
             << " 检查更新=" << *config_.checkUpdate;
    if (core_) {
        applyCoreSettings();
    }
}

void ArchtypeEngine::applyCoreSettings() {
    // 前两页是排好序的整句、词和句首词，后面把能对上的同音词、句首词、单字全补上，
    // 翻页能一直翻到生僻字（每次按键多花不到 1ms）
    archtype_engine_set_max_candidates(core_, kMaxCandidates);
    archtype_engine_set_fuzzy(core_, fuzzy_ ? 1 : 0);
    archtype_engine_set_fuzzy_groups(core_, fuzzyGroups_);
    archtype_engine_set_shuangpin(core_, shuangpin_ ? 1 : 0);
    if (archtype_engine_set_shuangpin_scheme(core_, shuangpinScheme_.c_str()) != 0) {
        WLOG_W() << "ArchType: 不认识双拼方案 " << shuangpinScheme_ << "，用自然码";
        archtype_engine_set_shuangpin_scheme(core_, "ziranma");
    }
    archtype_engine_set_typo_correction(core_, typo_ ? 1 : 0);
    archtype_engine_set_rare_ext_chars(core_, rareExt_ ? 1 : 0);
    if (traditional_ && !conversionLoaded_) {
        const std::string s2t = findDataFile("ARCHTYPE_S2T", "s2t.tsv");
        const int loaded =
            s2t.empty() ? -1 : archtype_engine_load_conversion(core_, s2t.c_str());
        if (loaded > 0) {
            conversionLoaded_ = true;
            WLOG_I() << "ArchType: 载入简繁表 " << loaded << " 条";
        } else {
            WLOG_W() << "ArchType: 简繁表载入失败 " << s2t;
        }
    }
    archtype_engine_set_traditional(core_, traditional_ && conversionLoaded_ ? 1 : 0);
    if (wubi_ && !wubiLoaded_) {
        // 选了五笔才载（九万多条，约 20ms、3MB）；没有码表时会话照全拼打，候选框上方说为什么
        const std::string wubi = findDataFile("ARCHTYPE_WUBI", "wubi86.tsv");
        const int loaded = wubi.empty() ? -1 : archtype_engine_load_wubi(core_, wubi.c_str());
        if (loaded > 0) {
            wubiLoaded_ = true;
            WLOG_I() << "ArchType: 载入五笔码表 " << wubi << "，" << loaded << " 条";
        } else {
            WLOG_W() << "ArchType: 五笔码表载入失败 " << (wubi.empty() ? "（没有 wubi86.tsv）" : wubi)
                     << "，先按全拼打";
        }
    }

    if (const std::string custom = customWordsPath(); !custom.empty()) {
        const int loaded = archtype_engine_load_custom_words(core_, custom.c_str());
        if (loaded < 0) {
            WLOG_W() << "ArchType: 自定义词载入失败 " << custom << "："
                         << (archtype_last_error() ? archtype_last_error() : "unknown");
        } else if (loaded > 0) {
            WLOG_I() << "ArchType: 载入自定义词 " << loaded << " 条";
        }
    }
}

std::string ArchtypeEngine::customWordsPath() const {
    if (const char *env = std::getenv("ARCHTYPE_CUSTOM"); env && *env) {
        return env;
    }
    const std::string dir = userDataDir();
    return dir.empty() ? std::string() : dir + "/custom.tsv";
}

bool ArchtypeEngine::isVoiceKey(const fcitx::Key &key) const {
    if (voiceKey_.sym() == FcitxKey_None || key.sym() != voiceKey_.sym()) {
        return false;
    }
    // 单按修饰键（右 Alt 等）不管状态位；松开时状态位里会带着它自己
    if (voiceKey_.isModifier() || voiceKeyDown_) {
        return true;
    }
    const fcitx::KeyStates mask{fcitx::KeyState::SimpleMask};
    return (key.states() & mask) == (voiceKey_.states() & mask);
}

bool ArchtypeEngine::isEmojiKey(const fcitx::Key &key) const {
    return emojiKey_.sym() != FcitxKey_None && key.checkKeyList(fcitx::KeyList{emojiKey_});
}

uint32_t ArchtypeEngine::modeKeyAction(const fcitx::Key &key) const {
    auto is = [&key](const fcitx::Key &want) {
        return want.sym() != FcitxKey_None && key.checkKeyList(fcitx::KeyList{want});
    };
    return is(translateModeKey_) ? ARCHTYPE_KEY_ACTION_TOGGLE_TRANSLATE
           : is(toggleKey_)      ? ARCHTYPE_KEY_ACTION_TOGGLE_MODE
                                 : ARCHTYPE_KEY_ACTION_NONE;
}

bool ArchtypeEngine::canToggleMode() const {
    return shiftToggle_ != ArchtypeShiftToggle::Off || toggleKey_.sym() != FcitxKey_None;
}

ArchtypeEngine::~ArchtypeEngine() {
    if (loading_.valid()) {
        core_ = loading_.get();
    }
    // 会话借用引擎和翻译器：先让它们放开，再释放（ABI.md：要比会话活得久）
    instance_->inputContextManager().foreach([this](fcitx::InputContext *ic) {
        auto *state = ic->propertyFor(&factory_);
        archtype_session_set_translator(state->session_, nullptr);
        archtype_session_set_engine(state->session_, nullptr);
        return true;
    });
    if (core_) {
        maybeSaveUserDict(true);
        archtype_engine_free(core_);
        core_ = nullptr;
    }
    flushStats();
    if (stats_) {
        archtype_stats_free(stats_);
        stats_ = nullptr;
    }
    voiceTimer_.reset();
    if (voice_) {
        archtype_voice_free(voice_);
        voice_ = nullptr;
    }
    translateTimer_.reset();
    if (translator_) {
        archtype_translate_free(translator_);
        translator_ = nullptr;
    }
}

bool ArchtypeEngine::coreReady() {
    if (core_) {
        return true;
    }
    if (loading_.valid() &&
        loading_.wait_for(std::chrono::seconds(0)) == std::future_status::ready) {
        core_ = loading_.get();
        if (wantedLexicon_ != lexicon_ || fileStamp(lexicon_) != lexiconStamp_) {
            // 载入途中配置换了词库：这个不用了，换新的
            maybeSwitchLexicon();
            return false;
        }
        if (core_) {
            applyCoreSettings();
        }
    }
    return core_ != nullptr;
}

std::string ArchtypeEngine::userDictPath() const {
    if (const char *env = std::getenv("ARCHTYPE_USER_DICT"); env && *env) {
        return env;
    }
    const std::string dir = userDataDir();
    if (dir.empty()) {
        return {};
    }
    std::error_code ec;
    std::filesystem::create_directories(dir, ec);
    return dir + "/user.tsv";
}

void ArchtypeEngine::maybeSaveUserDict(bool force) {
    if (!core_ || (!force && unsavedCommits_ < kSaveEvery) || unsavedCommits_ == 0) {
        return;
    }
    const std::string path = userDictPath();
    if (path.empty()) {
        return;
    }
    const bool dictSaved = archtype_engine_save_user_dict(core_, path.c_str()) == 0;
    const std::string history = historyPathFor(path);
    const bool historySaved =
        !history.empty() && archtype_engine_save_history(core_, history.c_str()) == 0;
    if (!historySaved) {
        WLOG_W() << "ArchType: 输入习惯保存失败 " << history << "："
                     << (archtype_last_error() ? archtype_last_error() : "unknown");
    }
    if (dictSaved && historySaved) {
        unsavedCommits_ = 0;
    }
}

void ArchtypeEngine::finishCommit(fcitx::InputContext *ic, ArchtypeState *state,
                                 const ArchtypeFrame *frame) {
    // 会话已经清空编辑区、记过选词和上文、补好了空格；这里上屏、攒着存用户词库
    const std::string text = frame && frame->commit ? frame->commit : "";
    const bool learned = frame && frame->learned;
    if (ic && !text.empty()) {
        // 会话已经补好了空格；互译时攒原文也是会话做的，这里上屏的是译文、原文或不在互译时的字
        recordStats(text, frame);
        ic->commitString(text);
    }
    if (learned && core_ && !text.empty()) {
        ++unsavedCommits_;
    }
    // 会话上屏时已经清好了它上屏的那部分，这里从不 reset 会话（reset 会把还在打的丢掉）：只丢掉抄的候选，
    // 会话还在打字就重新抄（五笔顶字：首选上屏，新打的字母开始下一个码；离开「译」：拼音留着接着打）
    state->forgetCandidates();
    if (state->composing()) {
        syncView(state);
    }
    if (learned) {
        maybeSaveUserDict(false);
    }
}

void ArchtypeEngine::commitInput(fcitx::InputContext *ic, ArchtypeState *state) {
    if (!ic) {
        return;
    }
    finishCommit(ic, state, archtype_session_reset(state->session_, 1));
}

void ArchtypeEngine::syncSession(ArchtypeState *state) {
    archtype_session_set_engine(state->session_, core_);
    archtype_session_set_translator(state->session_, translator_);
    ArchtypeSessionConfig config{};
    config.size = sizeof(config);
    config.page_size = static_cast<uint32_t>(pageSize_);
    config.calculator = calculator_ ? 1 : 0;
    config.date_time = dateTime_ ? 1 : 0;
    config.emoji_suggest = emojiSuggest_ ? 1 : 0;
    config.horizontal = horizontal_ ? 1 : 0;
    config.arrow_keys = arrowKeys_ == ArchtypeArrowKeys::Caret ? ARCHTYPE_ARROW_KEYS_CARET
                                                             : ARCHTYPE_ARROW_KEYS_CANDIDATES;
    config.page_keys = (pageMinus_ ? ARCHTYPE_PAGE_KEYS_MINUS : 0u) |
                       (pageBracket_ ? ARCHTYPE_PAGE_KEYS_BRACKET : 0u) |
                       (pageComma_ ? ARCHTYPE_PAGE_KEYS_COMMA : 0u) |
                       (pageTab_ ? ARCHTYPE_PAGE_KEYS_TAB : 0u);
    config.select_keys = selectKeys_ == ArchtypeSelectKeys::Semicolon ? ARCHTYPE_SELECT_KEYS_SEMICOLON
                         : selectKeys_ == ArchtypeSelectKeys::SemicolonQuote
                             ? ARCHTYPE_SELECT_KEYS_SEMICOLON_QUOTE
                             : ARCHTYPE_SELECT_KEYS_OFF;
    config.pick_char = pickChar_ ? 1 : 0;
    config.punct_style = punctStyle_ == ArchtypePunctStyle::Halfwidth ? 1 : 0;
    config.auto_space = static_cast<uint32_t>(autoSpaceFlags_);
    config.shift_toggle = shiftToggle_ == ArchtypeShiftToggle::Left    ? ARCHTYPE_SHIFT_TOGGLE_LEFT
                          : shiftToggle_ == ArchtypeShiftToggle::Right ? ARCHTYPE_SHIFT_TOGGLE_RIGHT
                          : shiftToggle_ == ArchtypeShiftToggle::Off   ? ARCHTYPE_SHIFT_TOGGLE_OFF
                                                                      : ARCHTYPE_SHIFT_TOGGLE_BOTH;
    config.translate_mode = translateAvailable() ? 1 : 0;
    config.translate_key = translateModeKey_.sym() != FcitxKey_None ? 1 : 0;
    config.toolbar = toolbar_ ? 1 : 0;
    config.clipboard_history = clipboardHistory_ ? 1 : 0;
    config.shuangpin_keys = shuangpinShowKeys_ ? 1 : 0;
    config.wubi = wubi_ ? 1 : 0;
    config.wubi_auto_commit = *config_.wubiAutoCommit ? 1 : 0;
    config.wubi_hint = *config_.wubiHint ? 1 : 0;
    archtype_session_configure(state->session_, &config);
}

const ArchtypeFrame *ArchtypeEngine::sessionKey(fcitx::InputContext *ic, ArchtypeState *state,
                                              const fcitx::Key &key, uint32_t action) {
    coreReady();
    syncSession(state);
    syncMode(ic, state);
    uint32_t hints = 0;
    // 密码框、网址框、邮箱框不自动补空格；密码框里不翻译（「译」里也照常上屏、不往外发）
    if (!ic || ic->capabilityFlags().testAny(fcitx::CapabilityFlags{
                   fcitx::CapabilityFlag::PasswordOrSensitive, fcitx::CapabilityFlag::Url,
                   fcitx::CapabilityFlag::Email})) {
        hints |= ARCHTYPE_KEY_HINT_NO_AUTO_SPACE;
    }
    if (ic && !translatable(ic)) {
        hints |= ARCHTYPE_KEY_HINT_NO_TRANSLATE;
    }
    // 读得到周围文字时会话拿光标前的字核对自己记的上文（在应用里删过、改过就作废）
    const auto prev = prevChar(ic);
    if (prev) {
        hints |= ARCHTYPE_KEY_HINT_SURROUNDING;
    }
    const ArchtypeFrame *frame =
        state->sessionKey(static_cast<uint32_t>(key.sym()), static_cast<uint32_t>(key.states()),
                          action, prev.value_or(0), hints, nowMs());
    if (frame && frame->wake_after_ms > 0) {
        scheduleTick(ic, frame->wake_after_ms);
    }
    return frame;
}

std::optional<uint32_t> ArchtypeEngine::prevChar(fcitx::InputContext *ic) const {
    // 应用报告了周围文字才知道光标前是什么；选中了文字时看选区前面那个字
    if (!ic || !ic->capabilityFlags().test(fcitx::CapabilityFlag::SurroundingText)) {
        return std::nullopt;
    }
    const auto &surrounding = ic->surroundingText();
    if (!surrounding.isValid()) {
        return std::nullopt;
    }
    const unsigned pos = std::min(surrounding.cursor(), surrounding.anchor());
    const std::string &before = surrounding.text();
    if (pos > fcitx::utf8::length(before)) {
        return std::nullopt;
    }
    if (pos == 0) {
        return 0;
    }
    const auto offset = fcitx::utf8::ncharByteLength(before.begin(), pos - 1);
    const uint32_t prev = fcitx::utf8::getChar(before.begin() + offset, before.end());
    return fcitx::utf8::isValidChar(prev) ? std::optional<uint32_t>(prev) : std::nullopt;
}

uint32_t ArchtypeEngine::keyAction(const fcitx::Key &key) const {
    const auto sym = key.sym();
    if (sym == FcitxKey_Up || sym == FcitxKey_Down || sym == FcitxKey_Left ||
        sym == FcitxKey_Right) {
        return ARCHTYPE_KEY_ACTION_NONE;
    }
    const auto &global = instance_->globalConfig();
    if (key.checkKeyList(global.defaultPrevPage())) {
        return ARCHTYPE_KEY_ACTION_PREV_PAGE;
    }
    if (key.checkKeyList(global.defaultNextPage())) {
        return ARCHTYPE_KEY_ACTION_NEXT_PAGE;
    }
    if (key.checkKeyList(global.defaultPrevCandidate())) {
        return ARCHTYPE_KEY_ACTION_PREV_CANDIDATE;
    }
    if (key.checkKeyList(global.defaultNextCandidate())) {
        return ARCHTYPE_KEY_ACTION_NEXT_CANDIDATE;
    }
    return ARCHTYPE_KEY_ACTION_NONE;
}

void ArchtypeEngine::applySessionFrame(fcitx::InputContext *ic, ArchtypeState *state,
                                      const ArchtypeFrame *frame) {
    if (frame && frame->size >= offsetof(ArchtypeFrame, forgot) + sizeof(frame->forgot) &&
        frame->forgot) {
        // Ctrl+Delete 忘掉了学错的词：马上存，免得 fcitx5 退出前没存上、重启又回来
        ++unsavedCommits_;
        maybeSaveUserDict(true);
    }
    if (frame && frame->commit) {
        // 学没学到词看 frame->learned；选了 emoji 由会话记进最近用过。
        // 候选位置从 frame->choice_index 拿（1.11），统计用。
        finishCommit(ic, state, frame);
        updateInputPanel(ic, state);
        return;
    }
    if (!state->composing()) {
        state->forgetCandidates();
        updateInputPanel(ic, state);
        return;
    }
    const bool wasExpanded = state->expanded_;
    const bool recopied = syncView(state);
    if (recopied || wasExpanded || state->expanded_) {
        updateInputPanel(ic, state);
        return;
    }
    // 编辑区里的光标动了（左右键移进拼音，#129）：重画编辑区。只在左右键移光标时看
    //（「译」里画的是原文框和译文，光标一直在末尾）
    if (arrowKeys_ == ArchtypeArrowKeys::Caret && !translating(ic) && frame && frame->preedit) {
        const fcitx::Text &shown =
            inlinePreedit(ic) ? ic->inputPanel().clientPreedit() : ic->inputPanel().preedit();
        if (shown.cursor() != frame->preedit_cursor) {
            updateInputPanel(ic, state);
            return;
        }
    }
    // 只是翻页、挪候选光标：原地摆好候选框
    if (auto *list = dynamic_cast<ArchtypeSessionList *>(ic->inputPanel().candidateList().get())) {
        list->mirror(state->panelPage_, state->panelCursor_);
        ic->updateUserInterface(fcitx::UserInterfaceComponent::InputPanel);
    } else {
        updateInputPanel(ic, state);
    }
}

bool ArchtypeEngine::syncView(ArchtypeState *state) {
    ArchtypeSessionView view{};
    view.size = sizeof(view);
    if (archtype_session_view(state->session_, &view) != 0) {
        return false;
    }
    const bool recopy = view.generation != state->candGeneration_;
    if (recopy) {
        refreshCandidates(state);
    }
    state->candGeneration_ = view.generation;
    state->panelPage_ = view.page;
    state->panelCursor_ = view.cursor;
    state->expanded_ = view.expanded != 0 && !state->candidates_.empty();
    state->gridCursor_ = view.grid_cursor;
    return recopy;
}

void ArchtypeEngine::refreshCandidates(ArchtypeState *state) {
    state->candidates_.clear();
    state->candGeneration_ = UINT64_MAX;
    state->expanded_ = false;
    state->gridCursor_ = 0;
    state->panelPage_ = 0;
    state->panelCursor_ = 0;
    const bool panelOpen = state->panel() != ARCHTYPE_TOOL_NONE;
    if (!panelOpen && (state->remaining().empty() || !coreReady())) {
        return;
    }
    // 候选（引擎候选 + 计算器、emoji 推荐、数字按键 emoji、日期时间，或者开着的工具栏 / 表情面板的条目）由会话算，
    // fcitx5 的候选框要整个列表，抄一份过来；点候选按这份拷贝的序号调会话
    syncSession(state);
    ArchtypeSessionView view{};
    view.size = sizeof(view);
    if (archtype_session_view(state->session_, &view) == 0) {
        state->candGeneration_ = view.generation;
        state->panelPage_ = view.page;
        state->panelCursor_ = view.cursor;
        state->gridCursor_ = view.grid_cursor;
    }
    const std::size_t total = archtype_session_candidate_total(state->session_);
    state->candidates_.reserve(total);
    for (std::size_t i = 0; i < total; ++i) {
        ArchtypeCandidateInfo info{};
        info.size = sizeof(info);
        if (archtype_session_candidate_info(state->session_, i, &info) != 0) {
            continue;
        }
        ArchtypeCandidate candidate{info.text, info.pinyin, info.consumed};
        candidate.comment = info.comment;
        candidate.plain = info.kind == ARCHTYPE_CAND_PLAIN;
        if (info.kind == ARCHTYPE_CAND_EMOJI) {
            candidate.emoji = true;
            candidate.commit = info.commit;
            candidate.emojiText = info.emoji;
        }
        state->candidates_.push_back(std::move(candidate));
    }
}

std::string ArchtypeEngine::recentEmojiPath() const {
    const std::string dir = userDataDir();
    return dir.empty() ? std::string() : dir + "/emoji-recent.txt";
}

void ArchtypeEngine::sampleClipboard(fcitx::InputContext *ic) {
    if (!clipboardHistory_ || !ic) {
        return;
    }
    // 密码框里不记（clipboard 插件自己也不记标成密码的内容）
    if (ic->capabilityFlags().testAny(fcitx::CapabilityFlag::PasswordOrSensitive)) {
        return;
    }
    auto *addon = instance_->addonManager().addon("clipboard");
    if (!addon) {
        return;
    }
    const std::string text = addon->call<fcitx::IClipboard::clipboard>(ic);
    if (!text.empty()) {
        archtype_clipboard_remember(text.c_str());
    }
}

void ArchtypeEngine::fillToolPanel(fcitx::InputPanel &panel, ArchtypeState *state) {
    // 工具栏（菜单、剪贴板、常用语）：竖排一列，一页 9 个，数字键选这一页的
    constexpr std::size_t kToolPageSize = 9;
    const std::size_t n = state->candidates_.size();
    if (n == 0) {
        return;
    }
    const std::size_t cursor = std::min(state->panelCursor_, n - 1);
    auto list = std::make_unique<fcitx::CommonCandidateList>();
    list->setLayoutHint(fcitx::CandidateLayoutHint::Vertical);
    list->setPageSize(static_cast<int>(kToolPageSize));
    list->setSelectionKey(fcitx::Key::keyListFromString("1 2 3 4 5 6 7 8 9"));
    for (std::size_t i = 0; i < n; ++i) {
        list->append<ArchtypeToolWord>(this, state, i, state->candidates_[i].text);
    }
    // setGlobalCursorIndex 只设光标、不翻页：先翻到光标那一页，不然光标走到后面几页时看不到
    list->setPage(static_cast<int>(cursor / kToolPageSize));
    list->setGlobalCursorIndex(static_cast<int>(cursor));
    panel.setCandidateList(std::move(list));
}

void ArchtypeEngine::choosePanelItem(fcitx::InputContext *ic, ArchtypeState *state,
                                    std::size_t index) {
    // 点了工具栏 / 表情面板的一项：菜单进下一层，别的上屏（会话做，表情记进最近用过）
    if (!ic) {
        return;
    }
    if (state->panel() == ARCHTYPE_TOOL_MENU) {
        sampleClipboard(ic);
    }
    const ArchtypeFrame *frame = archtype_session_choose(state->session_, index);
    if (frame && frame->handled) {
        applySessionFrame(ic, state, frame);
    }
}

/// 还没选词的拼音，按第一个用完它的候选分好音节（hengxiang -> heng'xiang）。
/// 显示的还是用户打的字母；英文、双拼等切不了的原样显示。
std::string ArchtypeEngine::segmentedRemaining(ArchtypeState *state) {
    // 会话的编辑区 = 已选的词 + 分好音节的剩余部分（双拼显示成全拼），这里只要后半截
    syncSession(state);
    const ArchtypeFrame *frame = archtype_session_frame(state->session_);
    const std::string selected = state->selectedText();
    const std::string shown = frame && frame->preedit ? frame->preedit : "";
    if (shown.compare(0, selected.size(), selected) == 0) {
        return shown.substr(selected.size());
    }
    return state->remaining();
}

bool ArchtypeEngine::inlinePreedit(fcitx::InputContext *ic) const {
    return instance_->globalConfig().preeditEnabledByDefault() &&
           ic->capabilityFlags().test(fcitx::CapabilityFlag::Preedit);
}

void ArchtypeEngine::sendPreedit(fcitx::InputContext *ic,
                                const fcitx::Text &preedit) {
    auto &panel = ic->inputPanel();
    if (inlinePreedit(ic)) {
        setClientPreedit(panel, preedit);
    } else {
        panel.setPreedit(preedit);
    }
}

void ArchtypeEngine::updateInputPanel(fcitx::InputContext *ic,
                                     ArchtypeState *state) {
    auto &panel = ic->inputPanel();
    panel.reset();

    if (const int open = state->panel(); open != ARCHTYPE_TOOL_NONE) {
        // 工具栏 / 表情面板：条目、光标、提示、搜索词都是会话给的
        syncView(state);
        const ArchtypeFrame *frame = archtype_session_frame(state->session_);
        fcitx::Text preedit;
        preedit.append(frame && frame->preedit ? frame->preedit : "", fcitx::TextFormatFlag::Underline);
        preedit.setCursor(static_cast<int>(preedit.textLength()));
        sendPreedit(ic, preedit);
        if (frame && frame->aux) {
            panel.setAuxUp(fcitx::Text(frame->aux));
        }
        if (!state->candidates_.empty()) {
            if (open == ARCHTYPE_TOOL_EMOJI || open == ARCHTYPE_TOOL_SYMBOLS) {
                fillGridPanel(panel, state);
            } else {
                fillToolPanel(panel, state);
            }
        }
        ic->updatePreedit();
        ic->updateUserInterface(fcitx::UserInterfaceComponent::InputPanel);
        return;
    }

    const bool translate = translating(ic) && translator_;
    if (!state->composing()) {
        if (translate) {
            fillTranslateView(ic, state, panel);
        }
        ic->updatePreedit();
        ic->updateUserInterface(fcitx::UserInterfaceComponent::InputPanel);
        return;
    }

    if (translate) {
        // 原文和拼音在候选框里，输入框里是译文
        fillTranslateView(ic, state, panel);
    } else {
        // 编辑区：已选的词 + 还没选的拼音（带下划线）；光标位置会话给（左右键能移到拼音中间，#129）
        fcitx::Text preedit;
        preedit.append(state->selectedText());
        preedit.append(segmentedRemaining(state), fcitx::TextFormatFlag::Underline);
        const ArchtypeFrame *frame = archtype_session_frame(state->session_);
        const std::size_t cursor = frame && frame->preedit ? frame->preedit_cursor : preedit.textLength();
        preedit.setCursor(static_cast<int>(std::min(cursor, preedit.textLength())));
        sendPreedit(ic, preedit);
        if (frame && frame->aux) {
            // 五笔的「空码」、选了五笔却没有码表（可以没有候选，只有这一行）
            panel.setAuxUp(fcitx::Text(frame->aux));
        }
    }

    if (state->expanded_ && !state->candidates_.empty()) {
        fillGridPanel(panel, state);
    } else if (!state->candidates_.empty()) {
        auto candidates = std::make_unique<ArchtypeSessionList>(state);
        candidates->setLayoutHint(horizontal_
                                      ? fcitx::CandidateLayoutHint::Horizontal
                                      : fcitx::CandidateLayoutHint::Vertical);
        candidates->setPageSize(static_cast<int>(pageSize_));
        candidates->setSelectionKey(fcitx::KeyList{
            fcitx::Key(FcitxKey_1), fcitx::Key(FcitxKey_2), fcitx::Key(FcitxKey_3),
            fcitx::Key(FcitxKey_4), fcitx::Key(FcitxKey_5), fcitx::Key(FcitxKey_6),
            fcitx::Key(FcitxKey_7), fcitx::Key(FcitxKey_8), fcitx::Key(FcitxKey_9)});
        candidates->setCursorPositionAfterPaging(
            fcitx::CursorPositionAfterPaging::ResetToFirst);
        for (std::size_t i = 0; i < state->candidates_.size(); ++i) {
            candidates->append<ArchtypeCandidateWord>(this, state, i,
                                                    shownCandidate(state->candidates_[i]));
        }
        // 页码和光标照会话的（展开面板里走到后面再收起时，会话翻到光标那一页）
        candidates->mirror(state->panelPage_,
                           std::min(state->panelCursor_, state->candidates_.size() - 1));
        panel.setCandidateList(std::move(candidates));
    }

    ic->updatePreedit();
    ic->updateUserInterface(fcitx::UserInterfaceComponent::InputPanel);
}

/// 展开的候选面板：竖排列表的每一项是一行（kGridCols 个词），一屏 kGridRows 行。
///
/// 照微信输入法的样子：光标所在行标 １–６，光标处的词用〔〕框起来；同一屏里各列
/// 按最宽的词补齐，这样上下键在同一列里走。classic UI 和 kimpanel 都只会画一行或
/// 一列，画不了真正的网格，所以用全角字符拼出来。
void ArchtypeEngine::fillGridPanel(fcitx::InputPanel &panel, ArchtypeState *state) {
    static const char *kLabels[] = {"\uFF11", "\uFF12", "\uFF13",
                                    "\uFF14", "\uFF15", "\uFF16"};
    static_assert(sizeof(kLabels) / sizeof(kLabels[0]) == kGridCols);
    const auto &all = state->candidates_;
    const std::size_t n = all.size();
    state->gridCursor_ = std::min(state->gridCursor_, n - 1);
    const std::size_t rows = (n + kGridCols - 1) / kGridCols;
    const std::size_t cursorRow = state->gridCursor_ / kGridCols;

    auto list = std::make_unique<fcitx::CommonCandidateList>();
    list->setLayoutHint(fcitx::CandidateLayoutHint::Vertical);
    list->setPageSize(static_cast<int>(kGridRows));
    list->setSelectionKey(fcitx::KeyList{}); // 不要行号：编号画在行里
    for (std::size_t pageStart = 0; pageStart < rows; pageStart += kGridRows) {
        const std::size_t pageEnd = std::min(rows, pageStart + kGridRows);
        std::size_t widths[kGridCols] = {};
        for (std::size_t r = pageStart; r < pageEnd; ++r) {
            for (std::size_t c = 0; c < kGridCols && r * kGridCols + c < n; ++c) {
                widths[c] = std::max(widths[c], displayWidth(shownCandidate(all[r * kGridCols + c]).toString()));
            }
        }
        for (std::size_t r = pageStart; r < pageEnd; ++r) {
            fcitx::Text line;
            for (std::size_t c = 0; c < kGridCols && r * kGridCols + c < n; ++c) {
                const std::size_t index = r * kGridCols + c;
                const bool here = index == state->gridCursor_;
                const fcitx::Text shown = shownCandidate(all[index]);
                std::string after = here ? "\u3015" : "";
                padTo(after, displayWidth(shown.toString()), widths[c]);
                line.append(std::string(r == cursorRow ? kLabels[c] : "\u3000") + (here ? "\u3014" : "\u3000"));
                for (std::size_t i = 0; i < shown.size(); ++i) {
                    line.append(shown.stringAt(i), shown.formatAt(i));
                }
                line.append(after + (here ? "" : "\u3000"));
            }
            list->append<ArchtypeGridRow>(this, state, r, std::move(line));
        }
    }
    // 先翻到光标所在的那一屏（setGlobalCursorIndex 不翻页），下键走到第 7 行以后才看得到
    list->setPage(static_cast<int>(cursorRow / kGridRows));
    list->setGlobalCursorIndex(static_cast<int>(cursorRow));
    panel.setCandidateList(std::move(list));
}

void ArchtypeEngine::selectGridRow(fcitx::InputContext *ic, ArchtypeState *state,
                                  std::size_t row) {
    if (!state || state->candidates_.empty()) {
        return;
    }
    const std::size_t index = state->gridCursor_ / kGridCols == row
                                  ? state->gridCursor_
                                  : row * kGridCols;
    commitCandidate(ic, state, std::min(index, state->candidates_.size() - 1));
}

namespace {

/// 会话第 index 个候选还是不是外壳显示的那个。
bool sameCandidate(ArchtypeState *state, std::size_t index, const ArchtypeCandidate &shown) {
    ArchtypeCandidateInfo info{};
    info.size = sizeof(info);
    if (archtype_session_candidate_info(state->session_, index, &info) != 0) {
        return false;
    }
    return shown.text == info.text && shown.pinyin == info.pinyin && shown.comment == info.comment &&
           shown.consumed == info.consumed && shown.plain == (info.kind == ARCHTYPE_CAND_PLAIN) &&
           shown.emoji == (info.kind == ARCHTYPE_CAND_EMOJI);
}

} // namespace

void ArchtypeEngine::commitCandidate(fcitx::InputContext *ic,
                                    ArchtypeState *state, std::size_t index) {
    if (!ic || !state || index >= state->candidates_.size()) {
        return;
    }
    if (state->panel() != ARCHTYPE_TOOL_NONE) {
        choosePanelItem(ic, state, index);
        return;
    }
    // 选词、上屏、学习由会话做；外壳的候选是会话候选的一份拷贝，序号一样
    const ArchtypeCandidate chosen = state->candidates_[index];
    if (!sameCandidate(state, index, chosen)) {
        // 会话的候选在显示以后变了（上文、设置、引擎变了又没刷新）：选同一个词，找不到就只刷新显示
        index = SIZE_MAX;
        const std::size_t total = archtype_session_candidate_total(state->session_);
        for (std::size_t i = 0; i < total; ++i) {
            if (sameCandidate(state, i, chosen)) {
                index = i;
                break;
            }
        }
        if (index == SIZE_MAX) {
            refreshCandidates(state);
            updateInputPanel(ic, state);
            return;
        }
    }
    const ArchtypeFrame *frame = archtype_session_choose(state->session_, index);
    if (!frame || !frame->handled) {
        return;
    }
    if (frame->commit) {
        finishCommit(ic, state, frame);
    } else {
        // 句首词进了已选栈，剩下的拼音继续选
        refreshCandidates(state);
    }
    updateInputPanel(ic, state);
}

void ArchtypeEngine::commitCurrent(fcitx::InputContext *ic, ArchtypeState *state) {
    // 和按空格一样：上屏光标处的候选，没有候选时上屏原始输入
    const ArchtypeFrame *frame = sessionKey(ic, state, fcitx::Key(FcitxKey_space));
    if (frame && frame->handled) {
        applySessionFrame(ic, state, frame);
    }
}

void ArchtypeEngine::keyEvent(const fcitx::InputMethodEntry & /*entry*/,
                             fcitx::KeyEvent &event) {
    auto *ic = event.inputContext();
    if (!ic) {
        return;
    }
    auto *state = ic->propertyFor(&factory_);
    const fcitx::Key key = event.key();
    const auto sym = key.sym();

    // 按键热力图：所有按键都经过这里（输入法消费掉的、还给应用的都算），松开也传，
    // 长按和合成的自动重复由核心去重。密码框在 recordKey 里排除。
    recordKey(ic, event.origKey(), !event.isRelease());

    if (voice_ && handleVoiceKey(ic, state, event)) {
        return;
    }

    // 单按 Shift 切中英文（ArchType 自己的英文模式），由会话判断。fcitx 的「临时切换输入法」热键
    // （默认左 Shift）生效时 fcitx 会先把 Shift 吃掉，这里就收不到，两者不会重复切换。
    if (handleShiftKey(ic, state, event)) {
        return;
    }
    if (event.isRelease()) {
        return;
    }


    if (isEmojiKey(key)) {
        // 表情面板快捷键：开着就关上；打了一半的拼音当搜索词（会话做）
        coreReady();
        syncSession(state);
        applySessionFrame(ic, state, archtype_session_toggle_emoji_panel(state->session_));
        event.filterAndAccept();
        return;
    }
    if (const uint32_t action = modeKeyAction(key); action && state->panel() == ARCHTYPE_TOOL_NONE) {
        // 另设的切换键（中 ↔ 英，在「译」里回中文）、互译键（进出「译」）：会话切；
        // 进不了「译」（没配好、密码框）时交给应用
        const ArchtypeFrame *frame = sessionKey(ic, state, key, action);
        if (frame && frame->handled) {
            followSessionMode(ic, state, frame);
            event.filterAndAccept();
        }
        return;
    }
    if (state->panel() != ARCHTYPE_TOOL_NONE) {
        // 工具栏 / 表情面板开着：按键都归它（会话管）；带 Ctrl / Alt / Super 的收起面板、交给应用
        if (state->panel() == ARCHTYPE_TOOL_MENU) {
            sampleClipboard(ic); // 下一步可能进「剪贴板」页
        }
        const ArchtypeFrame *frame = sessionKey(ic, state, key);
        if (frame && frame->handled) {
            applySessionFrame(ic, state, frame);
            event.filterAndAccept();
        } else {
            updateInputPanel(ic, state);
        }
        return;
    }

    // 互译模式、没在打拼音：回车上屏译文、Shift+回车上屏原文、Esc 清空、退格删原文、数字和符号进原文……
    // 都由会话处理
    if (translating(ic) && !state->composing() && !key.states().testAny(fcitx::KeyStates{
                                                      fcitx::KeyState::Ctrl, fcitx::KeyState::Alt,
                                                      fcitx::KeyState::Super})) {
        if (const ArchtypeFrame *frame = sessionKey(ic, state, key); frame && frame->handled) {
            applySessionFrame(ic, state, frame);
            event.filterAndAccept();
            return;
        }
    }

    // 英文模式：按键原样交给应用（字母、半角标点都直接上屏）；汉字后面打字母、数字要补空格时
    // 会话连空格一起上屏
    if (asciiMode_ && !state->composing()) {
        if (const ArchtypeFrame *frame = sessionKey(ic, state, key); frame && frame->handled) {
            applySessionFrame(ic, state, frame);
            event.filterAndAccept();
        }
        return;
    }

    // 带 Ctrl/Alt/Super 的组合键先问会话：打字途中 Ctrl+Delete、Ctrl+退格忘掉光标处学错的词；
    // 别的交给应用或 fcitx 的其他处理器（没在打字时会话借这一下知道应用里的字可能变了）
    if (key.states().testAny(fcitx::KeyStates{fcitx::KeyState::Ctrl, fcitx::KeyState::Alt,
                                              fcitx::KeyState::Super})) {
        if (const ArchtypeFrame *frame = sessionKey(ic, state, key); frame && frame->handled) {
            applySessionFrame(ic, state, frame);
            event.filterAndAccept();
        }
        return;
    }

    if (state->composing()) {
        // 只打了一个 v 再按空格打开工具栏：会话做（剪贴板页要的历史先读一次）
        if (sym == FcitxKey_space && state->input() == "v") {
            sampleClipboard(ic);
        }
        // 字母、隔音符、退格、Esc、空格、回车、数字、分号引号选词、V 模式算式、双拼分号、
        // 翻页、移动候选、展开面板都由会话处理。fcitx5 全局设置里的翻页、移动候选热键
        // （[Hotkey/PrevPage] 等，比如 , . 翻页）翻成动作一起交给会话：按了就算翻不动也吃掉，
        // 不让它变成标点把字上屏
        const bool wasExpanded = state->expanded_;
        const ArchtypeFrame *frame = sessionKey(ic, state, key, keyAction(key));
        if (frame && frame->handled) {
            applySessionFrame(ic, state, frame);
            event.filterAndAccept();
            return;
        }
        if (wasExpanded) {
            // 展开面板里按了会话不管的键（标点、Home……）：面板已经收起，重画再照常处理
            syncView(state);
            updateInputPanel(ic, state);
        }

        // 标点、符号（先上屏候选再上屏标点）也由会话处理了；会话不管的只剩下面这些
        // 编辑区里光标没法移动，这些键吃掉，免得应用里的光标乱跑
        if (key.isCursorMove() || sym == FcitxKey_Tab || sym == FcitxKey_Delete) {
            event.filterAndAccept();
            return;
        }
    }

    // 没在打字时也交给会话：字母（大写原样保留，Hello、iPhone 要用到）开始编辑区；
    // 标点换成中文标点（半角样式、数字后面的、没有中文写法的 / + 交给应用）；
    // 汉字后面打数字要补空格时会话自己上屏（第5章）
    // （打字途中的键上面已经交给过会话了，会话不管的交给应用）
    if (state->composing()) {
        return;
    }
    if (const ArchtypeFrame *frame = sessionKey(ic, state, key); frame && frame->handled) {
        applySessionFrame(ic, state, frame);
        event.filterAndAccept();
        return;
    }
}

void ArchtypeEngine::activate(const fcitx::InputMethodEntry & /*entry*/,
                             fcitx::InputContextEvent &event) {
    auto *ic = event.inputContext();
    if (!ic) {
        return;
    }
    auto *state = ic->propertyFor(&factory_);
    state->clear();
    clearTranslation(state);
    coreReady();
    // 从别的输入法切回来（不是换窗口获得焦点）、启动后第一次用：按「切到 ArchType 时」定中 / 英
    // （#158：切走时是英文，切回来还是英文，打不了中文）。换窗口不动，模式照旧全局保持；
    // 进出密码框时 fcitx5 自己换成键盘布局再换回来（CapabilityChanged），也不算用户切回来。
    bool switchedBack = false;
    if (event.type() == fcitx::EventType::InputContextSwitchInputMethod) {
        const auto &switched = static_cast<fcitx::InputContextSwitchInputMethodEvent &>(event);
        switchedBack = switched.reason() != fcitx::InputMethodSwitchedReason::CapabilityChanged;
    }
    if (switchedBack || !initialModeApplied_) {
        initialModeApplied_ = true;
        applyInitialMode(ic, state);
    }
    // 最近用过的 emoji 在核心里（所有输入框共用），存在 ~/.local/share/archtype/emoji-recent.txt
    archtype_recent_emoji_path(recentEmojiPath().c_str());
    sampleClipboard(ic);
    maybeCheckUpdate();
}

/// 后台的小事交给设置界面（archtype-config --startup）：第一次用时打开 ArchType 主题、
/// 检查更新（它自己看「自动检查更新」开关，最多一小时查一次，有新版本时发桌面通知）。
/// 插件这边在切到 ArchType 时叫它，同一个 fcitx5 进程里最多一小时叫一次。
void ArchtypeEngine::maybeCheckUpdate() {
#ifdef ARCHTYPE_CONFIG_TOOL
    if (std::getenv("ARCHTYPE_NO_UPDATE_CHECK")) {
        return;
    }
    const auto nowTime = std::chrono::steady_clock::now();
    if (updateCheckedAt_ && nowTime - *updateCheckedAt_ < std::chrono::hours(1)) {
        return;
    }
    updateCheckedAt_ = nowTime;
    std::error_code ec;
    if (std::filesystem::exists(ARCHTYPE_CONFIG_TOOL, ec)) {
        fcitx::startProcess({ARCHTYPE_CONFIG_TOOL, "--startup"});
    }
#endif
}

void ArchtypeEngine::deactivate(const fcitx::InputMethodEntry &entry,
                               fcitx::InputContextEvent &event) {
    // 打到一半切换输入法（比如按 Shift 切英文）：已选的词 + 原始字母原样上屏，
    // 不要把用户打的东西吞掉。失去焦点时放弃（reset）：这时再上屏，Qt 会送进新拿到焦点的控件，
    // XIM 的应用收不到；编辑区标了 DontCommit，工具包自己也不上屏。
    if (auto *ic = event.inputContext();
        ic && event.type() == fcitx::EventType::InputContextSwitchInputMethod) {
        auto *state = ic->propertyFor(&factory_);
        // 互译时拼音先进原文，攒着的原文再原样上屏（会话做）
        finishCommit(ic, state, archtype_session_reset(state->session_, 1));
    }
    if (voiceActive() && voiceIc_.get() == event.inputContext()) {
        cancelVoice();
    }
    reset(entry, event);
    maybeSaveUserDict(true);
    flushStats();
}

void ArchtypeEngine::reset(const fcitx::InputMethodEntry & /*entry*/,
                          fcitx::InputContextEvent &event) {
    auto *ic = event.inputContext();
    if (!ic) {
        return;
    }
    // 原文和拼音一样只在输入法里：应用重置输入框（点了别处、失去焦点）就丢掉，
    // 会话记的上文也作废（回来时光标可能在别处）。编辑区标了 DontCommit，工具包不会自己上屏。
    auto *state = ic->propertyFor(&factory_);
    state->clear();
    clearTranslation(state);
    ic->inputPanel().reset();
    ic->updatePreedit();
    ic->updateUserInterface(fcitx::UserInterfaceComponent::InputPanel);
}

const char *ArchtypeEngine::modeTitle() const {
    return translateMode_ ? "互译" : asciiMode_ ? "英文" : "中文";
}

const char *ArchtypeEngine::modeLabel() const {
    return translateMode_ ? "译" : asciiMode_ ? "英" : "中";
}

std::string ArchtypeEngine::subMode(const fcitx::InputMethodEntry & /*entry*/,
                                   fcitx::InputContext & /*ic*/) {
    return modeTitle();
}

std::string ArchtypeEngine::subModeLabelImpl(const fcitx::InputMethodEntry & /*entry*/,
                                            fcitx::InputContext & /*ic*/) {
    // 中文也要给：给空的话切回中文时提示框、托盘写的是输入法的名字「ArchType」，和「英」「译」对不上
    return modeLabel();
}

/// 按下 Shift 记下来，松开时中间没按别的键（也没按太久）就切中英文。
bool ArchtypeEngine::handleShiftKey(fcitx::InputContext *ic, ArchtypeState *state,
                                   fcitx::KeyEvent &event) {
    coreReady();
    syncSession(state);
    syncMode(ic, state);
    const fcitx::Key key = event.key();
    const auto sym = key.sym();
    const bool isShift = sym == FcitxKey_Shift_L || sym == FcitxKey_Shift_R;
    ArchtypeKey raw{};
    raw.size = sizeof(raw);
    raw.keysym = static_cast<uint32_t>(sym);
    raw.modifiers = static_cast<uint32_t>(key.states());
    raw.release = event.isRelease() ? 1 : 0;
    // 别的键只报给会话（按住 Shift 时按了它，这一下就不算单按），不让会话处理
    raw.action = isShift ? ARCHTYPE_KEY_ACTION_NONE : ARCHTYPE_KEY_ACTION_OBSERVE;
    // 松开时按键码认：xkb 选项（比如 grp:shifts_toggle）或 fcitx5 按输入法布局重新翻译时，
    // 松开那一下 Shift 可能翻成别的符号（ISO_Next_Group 等）
    raw.code = static_cast<uint32_t>(event.origKey().code());
    // 按住不放的自动重复不重新计时（key() 规整时去掉了 Repeat，要看 rawKey）
    raw.repeat = event.rawKey().states().test(fcitx::KeyState::Repeat) ? 1 : 0;
    raw.time_ms = static_cast<uint64_t>(fcitx::now(CLOCK_MONOTONIC) / 1000);
    const bool wasExpanded = state->expanded_;
    const ArchtypeFrame *frame = archtype_session_key(state->session_, &raw);
    if (!frame || !frame->mode_changed) {
        if (isShift) {
            // Shift 只走这一条路（再交给会话一次会把按下的时刻、键码冲掉）。展开面板里按下 Shift：
            // 会话已经把面板收起来了，重画；Shift 本身交给应用
            if (wasExpanded) {
                syncView(state);
                updateInputPanel(ic, state);
            }
            return true;
        }
        return false;
    }
    followSessionMode(ic, state, frame);
    event.filterAndAccept();
    return true;
}

void ArchtypeEngine::followSessionMode(fcitx::InputContext *ic, ArchtypeState *state,
                                      const ArchtypeFrame *frame) {
    // 打到一半切英文：已选的词 + 打的字母原样上屏；离开「译」：原文原样上屏、拼音留着（会话已经放进 frame）
    const int mode = frame->mode;
    const bool committed = frame->commit != nullptr;
    if (committed) {
        finishCommit(ic, state, frame);
    }
    switchMode(ic, state, mode == ARCHTYPE_MODE_ENGLISH, mode == ARCHTYPE_MODE_TRANSLATE, true);
    if (committed || state->composing()) {
        // 换好模式再画：离开「译」以后留着的拼音照普通的编辑区画，不再画成互译的原文框；
        // 带进「译」的拼音画成原文框
        updateInputPanel(ic, state);
    }
}

void ArchtypeEngine::applyInitialMode(fcitx::InputContext *ic, ArchtypeState *state) {
    if (initialMode_ == ArchtypeInitialMode::Last) {
        return;
    }
    // 切不回中文时不进英文
    const bool english = initialMode_ == ArchtypeInitialMode::English && canToggleMode();
    if (english == asciiMode_ && !translateMode_) {
        return;
    }
    switchMode(ic, state, english, false, false);
}

void ArchtypeEngine::syncMode(fcitx::InputContext *ic, ArchtypeState *state) {
    // 中 / 英 / 译是全局的（在一个应用里切了，别的应用里也是）；会话每个输入框一份，按键前对齐
    const int want = translateMode_ ? ARCHTYPE_MODE_TRANSLATE
                     : asciiMode_   ? ARCHTYPE_MODE_ENGLISH
                                    : ARCHTYPE_MODE_CHINESE;
    if (archtype_session_mode(state->session_) == want) {
        return;
    }
    auto set = [&](int mode) {
        const ArchtypeFrame *frame = archtype_session_set_mode(state->session_, mode);
        if (frame && frame->commit && ic) {
            finishCommit(ic, state, frame);
            updateInputPanel(ic, state);
        }
    };
    if (want == ARCHTYPE_MODE_TRANSLATE) {
        // 别的输入框里进的「译」：会话先到进「译」前的模式，在这里按互译键也回得去
        set(translateFrom_);
    }
    set(want);
}

void ArchtypeEngine::switchMode(fcitx::InputContext *ic, ArchtypeState *state, bool ascii,
                               bool translate, bool notify) {
    // 离开「译」时攒着的原文原样上屏：会话做（下面 syncMode 切会话的模式时放进 frame）
    if (ascii && !asciiMode_ && ic && state && state->composing()) {
        // 打到一半切英文：已选的词 + 打的字母原样上屏（和微信输入法一样）
        commitInput(ic, state);
        updateInputPanel(ic, state);
    }
    if (translate && !translateMode_) {
        translateFrom_ = asciiMode_ ? ARCHTYPE_MODE_ENGLISH : ARCHTYPE_MODE_CHINESE;
    }
    asciiMode_ = ascii;
    translateMode_ = translate;
    if (state) {
        syncMode(ic, state);
    }
    WLOG_I() << "ArchType: 切到" << modeTitle();
    if (!ic) {
        return;
    }
    if (notify) {
#ifdef ARCHTYPE_HAVE_CUSTOM_IM_INFO
        std::string label = modeLabel();
        if (translateMode_ && translator_) {
            label += " ";
            label += archtype_translate_tag(translator_, "");
        }
        instance_->showCustomInputMethodInformation(ic, label);
#else
        instance_->showInputMethodInformation(ic);
#endif
    }
    ic->updateUserInterface(fcitx::UserInterfaceComponent::StatusArea);
}

bool ArchtypeEngine::handleVoiceKey(fcitx::InputContext *ic, ArchtypeState *state,
                                   fcitx::KeyEvent &event) {
    const fcitx::Key key = event.key();
    if (isVoiceKey(key)) {
        if (event.isRelease()) {
            if (!voiceKeyDown_) {
                return false;
            }
            voiceKeyDown_ = false;
            event.filterAndAccept();
            if (voiceActive()) {
                archtype_voice_key_up(voice_);
                pollVoice();
            }
            return true;
        }
        if (voiceKeyDown_) {
            // 按住时的自动重复
            event.filterAndAccept();
            return true;
        }
        // 正在打拼音时不抢这个键
        if (state->composing() && !voiceActive()) {
            return false;
        }
        voiceKeyDown_ = true;
        event.filterAndAccept();
        if (!voiceActive()) {
            voiceIc_ = ic->watch();
            voiceShowing_ = true;
            voiceErrorUntil_ = 0;
        }
        archtype_voice_key_down(voice_);
        pollVoice();
        if (!voiceTimer_) {
            voiceTimer_ = instance_->eventLoop().addTimeEvent(
                CLOCK_MONOTONIC, fcitx::now(CLOCK_MONOTONIC) + kVoicePollUsec, kTimerAccuracyUsec,
                [this](fcitx::EventSourceTime *source, uint64_t) {
                    if (pollVoice()) {
                        source->setNextInterval(kVoicePollUsec);
                        source->setOneShot();
                    }
                    return true;
                });
        } else {
            voiceTimer_->setNextInterval(kVoicePollUsec);
            voiceTimer_->setOneShot();
        }
        return true;
    }

    if (!voiceActive() || event.isRelease()) {
        return false;
    }
    if (key.sym() == FcitxKey_Escape) {
        cancelVoice();
        event.filterAndAccept();
        return true;
    }
    // 修饰键单独按下（比如按住热键时碰到 Shift）不打断
    if (key.isModifier()) {
        return false;
    }
    // 说话途中打别的键：不要这次的语音，键照常处理
    cancelVoice();
    return false;
}

bool ArchtypeEngine::pollVoice() {
    if (!voice_ || !voiceShowing_) {
        return false;
    }
    auto *ic = voiceIc_.get();
    if (!ic) {
        // 输入框没了
        cancelVoice();
        return false;
    }
    checkOverlayAck(ic);
    const int phase = archtype_voice_poll(voice_);
    const std::string text = archtype_voice_text(voice_);
    switch (phase) {
    case ARCHTYPE_VOICE_LISTENING: {
        const int level = archtype_voice_level(voice_);
        if (voiceOverlay()) {
            // 浮条由 GNOME 扩展画，这里只留编辑区里识别到一半的字
            showVoice(ic, {}, text);
            sendVoiceOverlay(ic, kOverlayListening, "正在听…", level);
            return true;
        }
        // 不写字，只画麦克风 + 跟着音量滚动的波形
        voiceLevels_.push_back(level);
        if (voiceLevels_.size() > kVoiceBars) {
            voiceLevels_.erase(voiceLevels_.begin());
        }
        showVoice(ic, "🎙 " + voiceWave(voiceLevels_) + " ", text);
        return true;
    }
    case ARCHTYPE_VOICE_RECOGNIZING: {
        if (voiceOverlay()) {
            showVoice(ic, {}, text);
            sendVoiceOverlay(ic, kOverlayRecognizing, "识别中…", 0);
            return true;
        }
        static const char *const kDots[] = {"·    ", "··   ", "···  ", "···· ", "·····"};
        showVoice(ic, std::string("🎙 ") + kDots[(voiceFrame_++ / 3) % 5] + " ", text);
        return true;
    }
    case ARCHTYPE_VOICE_RESULT: {
        clearVoice(true);
        // 和工具栏、表情面板上屏的一样交给会话：补空格、记上文，互译时进原文
        auto *state = ic->propertyFor(&factory_);
        coreReady();
        syncSession(state);
        finishCommit(ic, state,
                     archtype_session_commit_text(state->session_, text.c_str(),
                                                 prevChar(ic).value_or(0)));
        // 上屏以后再复制，复制不成也不影响上屏；密码框里说的话不进剪贴板
        if (voiceCopy_ && !ic->capabilityFlags().testAny(fcitx::CapabilityFlag::PasswordOrSensitive) &&
            !archtype::copyToClipboard(text)) {
            WLOG_I() << "ArchType: 没能复制到剪贴板（要装 wl-clipboard，X11 下装 xclip 或 xsel）";
        }
        if (translating(ic)) {
            // 互译模式：说的话进了原文
            updateInputPanel(ic, state);
        }
        return false;
    }
    case ARCHTYPE_VOICE_ERROR:
        WLOG_I() << "ArchType: 语音 " << text;
        voiceErrorText_ = text;
        if (voiceOverlay()) {
            showVoice(ic, {}, {});
            sendVoiceOverlay(ic, kOverlayError, text, 0);
        } else {
            showVoice(ic, "🎙 " + text, {});
        }
        // 没下载模型的提示要去设置里操作，多留一会儿让人看清
        voiceErrorUntil_ = fcitx::now(CLOCK_MONOTONIC) +
                           (archtype_voice_model_state(voice_) == ARCHTYPE_VOICE_MODEL_MISSING
                                ? kVoiceHintUsec
                                : kVoiceErrorUsec);
        return true;
    default:
        if (voiceErrorUntil_ && fcitx::now(CLOCK_MONOTONIC) < voiceErrorUntil_) {
            // 浮条 1.5 秒收不到信号会自己收起：提示要留得更久时接着发
            if (voiceOverlay()) {
                sendVoiceOverlay(ic, kOverlayError, voiceErrorText_, 0);
            }
            return true;
        }
        clearVoice();
        return false;
    }
}

void ArchtypeEngine::showVoice(fcitx::InputContext *ic, const std::string &status,
                              const std::string &partial) {
    auto &panel = ic->inputPanel();
    panel.reset();
    if (translating(ic)) {
        // 互译模式：说的话接在原文框里攒着的原文后面，输入框里留着译文
        auto *state = ic->propertyFor(&factory_);
        const ArchtypeFrame *frame = archtype_session_frame(state->session_);
        const std::string source = frame && frame->translating ? frame->translate_source : "";
        const std::string translated = frame && frame->translating ? frame->translate_text : "";
        fcitx::Text box;
        box.append(source);
        box.append(partial, fcitx::TextFormatFlag::Underline);
        box.setCursor(static_cast<int>(box.textLength()));
        panel.setPreedit(box);
        if (inlinePreedit(ic) && !translated.empty()) {
            fcitx::Text shown;
            shown.append(translated, fcitx::TextFormatFlag::Underline);
            shown.setCursor(static_cast<int>(shown.textLength()));
            setClientPreedit(panel, shown);
        }
    } else if (!partial.empty()) {
        fcitx::Text preedit;
        preedit.append(partial, fcitx::TextFormatFlag::Underline);
        preedit.setCursor(static_cast<int>(preedit.textLength()));
        sendPreedit(ic, preedit);
    }
    if (!status.empty()) {
        panel.setAuxUp(fcitx::Text(status));
    }
    ic->updatePreedit();
    ic->updateUserInterface(fcitx::UserInterfaceComponent::InputPanel);
}

void ArchtypeEngine::clearVoice(bool done) {
    if (voiceShowing_ && voiceOverlay()) {
        sendVoiceOverlay(voiceIc_.get(), done ? kOverlayDone : kOverlayHide,
                         done ? "识别完成" : "", 0);
    }
    voiceShowing_ = false;
    overlaySentAt_ = 0;
    overlayAcked_ = false;
    voiceLevels_.clear();
    voiceFrame_ = 0;
    voiceErrorUntil_ = 0;
    voiceErrorText_.clear();
    if (auto *ic = voiceIc_.get()) {
        // 回到说话前的样子（互译模式下还有原文和译文）
        updateInputPanel(ic, ic->propertyFor(&factory_));
    }
    voiceIc_.unwatch();
}

void ArchtypeEngine::cancelVoice() {
    if (voice_) {
        archtype_voice_cancel(voice_);
    }
    clearVoice();
}

// ---------------------------------------------------------------------------
// 互译模式
//
// 照微信输入法的样子：打的字（选好的词、标点、语音识别结果）攒成原文，显示在候选框里
// （当原文框用），不交给应用；原文停下来 0.5 秒（句末标点立刻）就发请求，译文边出边写在
// 应用的输入框里（编辑区，带下划线）。回车 / 空格上屏译文，Shift+回车上屏原文，Esc 清空。
// 这些都在会话里（archtype_session_tick 等），外壳只画、排定时器（会话的 wake_after_ms）。

bool ArchtypeEngine::translateAvailable() const {
    return translateEnabled_ && translator_ && archtype_translate_ready(translator_) == 1;
}

bool ArchtypeEngine::translating(fcitx::InputContext *ic) const {
    return translateMode_ && translator_ && translatable(ic);
}

bool ArchtypeEngine::translatable(fcitx::InputContext *ic) {
    // 密码框之类：照常打中文，不往外发
    if (!ic) {
        return false;
    }
    const auto flags = ic->capabilityFlags();
    return !flags.test(fcitx::CapabilityFlag::Password) &&
           !flags.test(fcitx::CapabilityFlag::Sensitive);
}

std::string ArchtypeEngine::translateKeyFile() const {
    if (const char *env = std::getenv("ARCHTYPE_TRANSLATE_KEY_FILE"); env && *env) {
        return env;
    }
    const std::string dir = userConfigDir();
    return dir.empty() ? std::string() : dir + "/translate.key";
}

void ArchtypeEngine::clearTranslation(ArchtypeState *state) {
    // 丢掉攒着的原文、取消这个输入框的请求（会话做）
    archtype_session_translate_clear(state->session_);
    if (translateIc_.get() && translateIc_.get()->propertyFor(&factory_) == state) {
        translateDrawn_.clear();
    }
}

void ArchtypeEngine::recordStats(const std::string &text, const ArchtypeFrame *frame) {
    if (!stats_ || text.empty()) {
        return;
    }
    const std::string src = typingSource();
    const std::string scheme = shuangpin_ ? typingScheme() : std::string();
    unsigned candidate = frame ? static_cast<unsigned>(frame->choice_index) : 0;
    if (src == "translate") {
        candidate = 0; // 互译上屏的不是候选
    }
    archtype_stats_commit(stats_, text.c_str(), src.c_str(),
                         scheme.empty() ? nullptr : scheme.c_str(),
                         static_cast<int>(candidate), statsNowMs());
}

void ArchtypeEngine::recordKey(fcitx::InputContext *ic, const fcitx::Key &key, bool down) {
    if (!stats_ || !ic) {
        return;
    }
    if (ic->capabilityFlags().test(fcitx::CapabilityFlag::PasswordOrSensitive)) {
        return;
    }
    archtype_stats_key(stats_, 0, static_cast<unsigned>(key.code()), down ? 1 : 0, statsNowMs());
}

void ArchtypeEngine::flushStats() {
    if (!stats_) {
        return;
    }
    if (archtype_stats_flush(stats_, statsNowMs()) != 0) {
        WLOG_W() << "ArchType: 打字统计写入失败："
                 << (archtype_last_error() ? archtype_last_error() : "unknown");
    }
}

std::string ArchtypeEngine::typingSource() const {
    if (translateMode_) {
        return "translate";
    }
    if (asciiMode_) {
        return "english";
    }
    return shuangpin_ ? "shuangpin" : "quanpin";
}

std::string ArchtypeEngine::typingScheme() const {
    std::string id = shuangpinScheme_;
    for (char &ch : id) {
        if (ch >= 'A' && ch <= 'Z') {
            ch = static_cast<char>(ch - 'A' + 'a');
        }
    }
    return id;
}

uint64_t ArchtypeEngine::nowMs() {
    return static_cast<uint64_t>(fcitx::now(CLOCK_MONOTONIC) / 1000);
}

void ArchtypeEngine::scheduleTick(fcitx::InputContext *ic, uint32_t ms) {
    if (!ic) {
        return;
    }
    if (translateIc_.get() != ic) {
        translateIc_ = ic->watch();
    }
    const uint64_t at = fcitx::now(CLOCK_MONOTONIC) + static_cast<uint64_t>(ms) * 1000;
    if (!translateTimer_) {
        translateTimer_ = instance_->eventLoop().addTimeEvent(
            CLOCK_MONOTONIC, at, kTimerAccuracyUsec,
            [this](fcitx::EventSourceTime *source, uint64_t) {
                const uint32_t next = tickTranslate();
                if (next > 0) {
                    source->setNextInterval(static_cast<uint64_t>(next) * 1000);
                    source->setOneShot();
                }
                return true;
            });
    } else {
        translateTimer_->setTime(at);
        translateTimer_->setOneShot();
    }
}

uint32_t ArchtypeEngine::tickTranslate() {
    auto *ic = translateIc_.get();
    if (!ic) {
        return 0;
    }
    auto *state = ic->propertyFor(&factory_);
    syncSession(state);
    const ArchtypeFrame *frame = archtype_session_tick(state->session_, nowMs());
    if (!frame) {
        return 0;
    }
    if (frame->commit) {
        // 按过回车，译好了：上屏译文（正在打的拼音留着，不清编辑区）
        recordStats(frame->commit, frame);
        ic->commitString(frame->commit);
        translateDrawn_.clear();
        updateInputPanel(ic, state);
        return frame->wake_after_ms;
    }
    // 结果变了才重画；只换编辑区和提示行：正在打拼音时候选列表（和翻到的页、光标）不动
    if (translating(ic) && translateSignature(frame) != translateDrawn_) {
        fillTranslateView(ic, state, ic->inputPanel());
        ic->updatePreedit();
        ic->updateUserInterface(fcitx::UserInterfaceComponent::InputPanel);
    }
    return frame->wake_after_ms;
}

std::string ArchtypeEngine::translateSignature(const ArchtypeFrame *frame) const {
    if (!frame || !frame->translating) {
        return {};
    }
    return std::string(frame->translate_source) + "|" + frame->translate_text + "|" +
           frame->translate_status + "|" + frame->translate_title + "|" +
           std::to_string(frame->translate_done);
}

void ArchtypeEngine::fillTranslateView(fcitx::InputContext *ic, ArchtypeState *state,
                                      fcitx::InputPanel &panel) {
    const ArchtypeFrame *frame = archtype_session_frame(state->session_);
    if (!frame || !frame->translating) {
        return;
    }
    translateDrawn_ = translateSignature(frame);
    if (frame->wake_after_ms > 0) {
        scheduleTick(ic, frame->wake_after_ms);
    }
    const std::string source = frame->translate_source;
    if (source.empty() && !state->composing()) {
        return;
    }
    // 原文框：攒着的原文 + 已选的词 + 还没选的拼音
    fcitx::Text box;
    box.append(source);
    if (state->composing()) {
        box.append(state->selectedText());
        box.append(segmentedRemaining(state), fcitx::TextFormatFlag::Underline);
    }
    box.setCursor(static_cast<int>(box.textLength()));
    panel.setPreedit(box);
    panel.setAuxUp(fcitx::Text(frame->translate_title));

    // 底下一行：状态，或者怎么上屏
    std::string status = frame->translate_status;
    const std::string shown = frame->translate_text;
    if (inlinePreedit(ic)) {
        fcitx::Text text;
        text.append(shown, fcitx::TextFormatFlag::Underline);
        text.setCursor(static_cast<int>(text.textLength()));
        setClientPreedit(panel, text);
    } else if (!shown.empty()) {
        // 应用不让把编辑区画在输入框里：译文写在候选框底下
        status = shown + (frame->translate_done ? "" : "…");
    }
    panel.setAuxDown(fcitx::Text(status));
}

class ArchtypeFactory final : public fcitx::AddonFactory {
public:
    fcitx::AddonInstance *create(fcitx::AddonManager *manager) override {
        if (!manager) {
            return nullptr;
        }
        return new ArchtypeEngine(manager->instance());
    }
};

// _V2 是 fcitx5 5.1.8 起才有的；Ubuntu 24.04 自带 5.1.7，只能用旧宏
#ifdef FCITX_ADDON_FACTORY_V2
FCITX_ADDON_FACTORY_V2(archtype, ArchtypeFactory)
#else
FCITX_ADDON_FACTORY(ArchtypeFactory)
#endif
