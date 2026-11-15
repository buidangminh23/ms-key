#ifndef MSKEY_CORE_H
#define MSKEY_CORE_H

#include <stdbool.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct MsKeyHandle MsKeyHandle;

typedef struct MsKeyAction {
    uint8_t kind;
    uint32_t backspaces;
    char *text;
} MsKeyAction;

MsKeyHandle *mskey_create(const char *config_json);
void mskey_destroy(MsKeyHandle *handle);

char *mskey_default_config_json(void);
char *mskey_get_config_json(MsKeyHandle *handle);
bool mskey_set_config_json(MsKeyHandle *handle, const char *config_json);
char *mskey_import_evkey_tree(const char *tree_text);

void mskey_set_enabled(MsKeyHandle *handle, bool enabled);
bool mskey_is_enabled(MsKeyHandle *handle);
void mskey_reset_word(MsKeyHandle *handle);
void mskey_reset_all(MsKeyHandle *handle);

MsKeyAction mskey_process_key(MsKeyHandle *handle, uint32_t codepoint);
MsKeyAction mskey_process_backspace(MsKeyHandle *handle);
MsKeyAction mskey_process_escape(MsKeyHandle *handle);

uint8_t mskey_behavior_for_app(MsKeyHandle *handle, const char *process_name);
char *mskey_convert_text(
    const char *input,
    uint8_t from_charset,
    uint8_t to_charset,
    uint8_t case_mode
);
char *mskey_convert_with_encoding(
    const char *input,
    uint8_t from_enc,
    uint8_t to_enc,
    uint8_t case_mode
);
uint8_t mskey_detect_charset(const char *input);
char *mskey_format_macro_text(const char *config_json);
char *mskey_parse_macro_text(const char *raw_text);
bool mskey_is_active_delimiter(MsKeyHandle *handle, uint32_t codepoint);
void mskey_set_clock_offset(MsKeyHandle *handle, int64_t offset_secs);

void mskey_free_string(char *ptr);
void mskey_free_action(MsKeyAction action);

#ifdef __cplusplus
}
#endif

#endif
