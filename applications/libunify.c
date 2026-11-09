#include "libunify.h"

#include <stdlib.h>
#include <string.h>

char *unify_run(const char *const argv[])
{
    gchar *standard_output = NULL;
    gint wait_status = 0;
    GError *error = NULL;

    if (!g_spawn_sync(NULL, (gchar **)argv, NULL,
                      G_SPAWN_SEARCH_PATH | G_SPAWN_STDERR_TO_DEV_NULL,
                      NULL, NULL, &standard_output, NULL, &wait_status, &error)) {
        g_warning("unify: cannot run %s: %s", argv[0], error->message);
        g_error_free(error);
        return NULL;
    }
    return standard_output;
}

gboolean unify_run_ok(const char *const argv[])
{
    gint wait_status = 0;
    GError *error = NULL;

    if (!g_spawn_sync(NULL, (gchar **)argv, NULL,
                      G_SPAWN_SEARCH_PATH | G_SPAWN_STDOUT_TO_DEV_NULL |
                          G_SPAWN_STDERR_TO_DEV_NULL,
                      NULL, NULL, NULL, NULL, &wait_status, &error)) {
        g_warning("unify: cannot run %s: %s", argv[0], error->message);
        g_error_free(error);
        return FALSE;
    }
    return g_spawn_check_wait_status(wait_status, NULL);
}

char *unify_tool(const char *name)
{
    const char *bindir = g_getenv("UNIFY_BINDIR");
    if (bindir != NULL && *bindir != '\0')
        return g_build_filename(bindir, name, NULL);
    return g_find_program_in_path(name);
}

char *unify_json_string(const char *json, const char *key)
{
    if (json == NULL || key == NULL)
        return NULL;

    gchar *needle = g_strdup_printf("\"%s\"", key);
    const char *cursor = strstr(json, needle);
    g_free(needle);
    if (cursor == NULL)
        return NULL;

    cursor += strlen(key) + 2;
    while (*cursor == ' ' || *cursor == '\t' || *cursor == '\n' || *cursor == ':')
        cursor++;
    if (*cursor != '"')
        return NULL;

    cursor++;
    const char *end = strchr(cursor, '"');
    if (end == NULL)
        return NULL;
    return g_strndup(cursor, (gsize)(end - cursor));
}

double unify_json_number(const char *json, const char *key, double fallback)
{
    if (json == NULL || key == NULL)
        return fallback;

    gchar *needle = g_strdup_printf("\"%s\"", key);
    const char *cursor = strstr(json, needle);
    g_free(needle);
    if (cursor == NULL)
        return fallback;

    cursor += strlen(key) + 2;
    while (*cursor == ' ' || *cursor == '\t' || *cursor == '\n' || *cursor == ':')
        cursor++;
    if (*cursor != '-' && (*cursor < '0' || *cursor > '9'))
        return fallback;
    return g_ascii_strtod(cursor, NULL);
}

gboolean unify_json_bool(const char *json, const char *key, gboolean fallback)
{
    if (json == NULL || key == NULL)
        return fallback;

    gchar *needle = g_strdup_printf("\"%s\"", key);
    const char *cursor = strstr(json, needle);
    g_free(needle);
    if (cursor == NULL)
        return fallback;

    cursor += strlen(key) + 2;
    while (*cursor == ' ' || *cursor == '\t' || *cursor == '\n' || *cursor == ':')
        cursor++;
    if (g_str_has_prefix(cursor, "true"))
        return TRUE;
    if (g_str_has_prefix(cursor, "false"))
        return FALSE;
    return fallback;
}

GtkWidget *unify_window(GtkApplication *app, const char *title, int width, int height)
{
    GtkWidget *window = gtk_application_window_new(app);
    gtk_window_set_title(GTK_WINDOW(window), title);
    gtk_window_set_default_size(GTK_WINDOW(window), width, height);

    GtkWidget *header = gtk_header_bar_new();
    GtkWidget *title_label = gtk_label_new(title);
    gtk_widget_add_css_class(title_label, "title");
    gtk_header_bar_set_title_widget(GTK_HEADER_BAR(header), title_label);
    gtk_window_set_titlebar(GTK_WINDOW(window), header);

    return window;
}

GtkWidget *unify_scrolled(GtkWidget *child)
{
    GtkWidget *scroller = gtk_scrolled_window_new();
    gtk_scrolled_window_set_child(GTK_SCROLLED_WINDOW(scroller), child);
    gtk_scrolled_window_set_policy(GTK_SCROLLED_WINDOW(scroller),
                                   GTK_POLICY_AUTOMATIC, GTK_POLICY_AUTOMATIC);
    gtk_widget_set_vexpand(scroller, TRUE);
    return scroller;
}

GtkWidget *unify_row(const char *title, const char *subtitle)
{
    GtkWidget *row = gtk_list_box_row_new();
    GtkWidget *box = gtk_box_new(GTK_ORIENTATION_VERTICAL, 2);
    gtk_widget_set_margin_top(box, 8);
    gtk_widget_set_margin_bottom(box, 8);
    gtk_widget_set_margin_start(box, 12);
    gtk_widget_set_margin_end(box, 12);

    GtkWidget *title_label = gtk_label_new(title != NULL ? title : "");
    gtk_label_set_xalign(GTK_LABEL(title_label), 0.0f);
    gtk_label_set_ellipsize(GTK_LABEL(title_label), PANGO_ELLIPSIZE_END);
    gtk_box_append(GTK_BOX(box), title_label);

    if (subtitle != NULL && *subtitle != '\0') {
        GtkWidget *subtitle_label = gtk_label_new(subtitle);
        gtk_label_set_xalign(GTK_LABEL(subtitle_label), 0.0f);
        gtk_label_set_wrap(GTK_LABEL(subtitle_label), TRUE);
        gtk_widget_add_css_class(subtitle_label, "dim-label");
        gtk_widget_add_css_class(subtitle_label, "caption");
        gtk_box_append(GTK_BOX(box), subtitle_label);
    }

    gtk_list_box_row_set_child(GTK_LIST_BOX_ROW(row), box);
    gtk_list_box_row_set_activatable(GTK_LIST_BOX_ROW(row), FALSE);
    return row;
}

GtkWidget *unify_heading(const char *text)
{
    GtkWidget *label = gtk_label_new(text);
    gtk_widget_add_css_class(label, "title-4");
    gtk_label_set_xalign(GTK_LABEL(label), 0.0f);
    gtk_widget_set_margin_top(label, 14);
    gtk_widget_set_margin_bottom(label, 4);
    gtk_widget_set_margin_start(label, 12);
    return label;
}

GtkWidget *unify_log_view(void)
{
    GtkWidget *view = gtk_text_view_new();
    gtk_text_view_set_editable(GTK_TEXT_VIEW(view), FALSE);
    gtk_text_view_set_cursor_visible(GTK_TEXT_VIEW(view), FALSE);
    gtk_text_view_set_monospace(GTK_TEXT_VIEW(view), TRUE);
    gtk_text_view_set_wrap_mode(GTK_TEXT_VIEW(view), GTK_WRAP_WORD_CHAR);
    return unify_scrolled(view);
}

void unify_log_append(GtkWidget *scrolled, const char *text)
{
    GtkWidget *view = gtk_scrolled_window_get_child(GTK_SCROLLED_WINDOW(scrolled));
    if (view == NULL || !GTK_IS_TEXT_VIEW(view))
        return;

    GtkTextBuffer *buffer = gtk_text_view_get_buffer(GTK_TEXT_VIEW(view));
    GtkTextIter end;
    gtk_text_buffer_get_end_iter(buffer, &end);
    gtk_text_buffer_insert(buffer, &end, text, -1);

    GtkTextMark *mark = gtk_text_buffer_create_mark(buffer, NULL, &end, FALSE);
    gtk_text_view_scroll_mark_onscreen(GTK_TEXT_VIEW(view), mark);
}

void unify_install_css(void)
{
    GtkCssProvider *provider = gtk_css_provider_new();
    const char *candidates[] = {
        "/usr/share/unify/unify.css",
        NULL
    };

    for (int index = 0; candidates[index] != NULL; index++) {
        if (!g_file_test(candidates[index], G_FILE_TEST_EXISTS))
            continue;
        gtk_css_provider_load_from_path(provider, candidates[index]);
        gtk_style_context_add_provider_for_display(
            gdk_display_get_default(), GTK_STYLE_PROVIDER(provider),
            GTK_STYLE_PROVIDER_PRIORITY_APPLICATION);
        break;
    }
    g_object_unref(provider);
}
