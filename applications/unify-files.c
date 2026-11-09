/* Unify File Manager — browse, open and reveal files. */

#include "libunify.h"

#include <stdlib.h>
#include <string.h>

typedef struct {
    GtkWidget *path;
    GtkWidget *list;
    GtkWidget *status;
    char *current;
} Files;

static void log_status(Files *files, const char *text)
{
    gtk_label_set_text(GTK_LABEL(files->status), text);
}

static void clear_list(Files *files)
{
    GtkListBoxRow *row;
    while ((row = gtk_list_box_get_row_at_index(GTK_LIST_BOX(files->list), 0)) != NULL)
        gtk_list_box_remove(GTK_LIST_BOX(files->list), GTK_WIDGET(row));
}

static void navigate(Files *files, const char *path)
{
    char *canonical = g_canonicalize_filename(path, NULL);
    GDir *directory = g_dir_open(canonical, 0, NULL);
    if (directory == NULL) {
        gchar *message = g_strdup_printf("Cannot open %s", canonical);
        log_status(files, message);
        g_free(message);
        g_free(canonical);
        return;
    }

    g_free(files->current);
    files->current = g_strdup(canonical);
    gtk_editable_set_text(GTK_EDITABLE(files->path), canonical);
    clear_list(files);

    GPtrArray *names = g_ptr_array_new_with_free_func(g_free);
    const char *name;
    while ((name = g_dir_read_name(directory)) != NULL) {
        if (name[0] == '.')
            continue;
        g_ptr_array_add(names, g_strdup(name));
    }
    g_dir_close(directory);
    g_ptr_array_sort(names, (GCompareFunc)g_strcmp0);

    for (guint index = 0; index < names->len; index++) {
        const char *entry = g_ptr_array_index(names, index);
        gchar *full = g_build_filename(canonical, entry, NULL);
        gboolean is_directory = g_file_test(full, G_FILE_TEST_IS_DIR);
        GtkWidget *row = unify_row(is_directory ? g_strdup_printf("%s/", entry) : entry,
                                   is_directory ? "Directory" : full);
        g_object_set_data_full(G_OBJECT(row), "full-path", full,
                               (GDestroyNotify)g_free);
        g_object_set_data(G_OBJECT(row), "is-directory", GINT_TO_POINTER(is_directory));
        gtk_list_box_append(GTK_LIST_BOX(files->list), row);
    }

    gchar *message = g_strdup_printf("%u entries", names->len);
    log_status(files, message);
    g_free(message);
    g_ptr_array_free(names, TRUE);
    g_free(canonical);
}

static void on_row_activated(GtkListBox *list, GtkListBoxRow *row, gpointer data)
{
    (void)list;
    Files *files = data;
    const char *full = g_object_get_data(G_OBJECT(row), "full-path");
    if (full == NULL)
        return;

    if (GPOINTER_TO_INT(g_object_get_data(G_OBJECT(row), "is-directory"))) {
        navigate(files, full);
        return;
    }

    gchar *command = g_strdup_printf("xdg-open '%s'", full);
    g_spawn_command_line_async(command, NULL);
    g_free(command);
    log_status(files, "Opened with the default application.");
}

static void on_up(GtkButton *button, gpointer data)
{
    (void)button;
    Files *files = data;
    char *parent = g_path_get_dirname(files->current);
    navigate(files, parent);
    g_free(parent);
}

static void on_go(GtkButton *button, gpointer data)
{
    (void)button;
    Files *files = data;
    navigate(files, gtk_editable_get_text(GTK_EDITABLE(files->path)));
}

static void on_activate(GtkApplication *application, gpointer data)
{
    (void)data;
    Files *files = g_new0(Files, 1);

    GtkWidget *window = unify_window(application, "Files", 620, 660);
    GtkWidget *content = gtk_box_new(GTK_ORIENTATION_VERTICAL, 0);

    GtkWidget *bar = gtk_box_new(GTK_ORIENTATION_HORIZONTAL, 6);
    gtk_widget_set_margin_top(bar, 10);
    gtk_widget_set_margin_bottom(bar, 6);
    gtk_widget_set_margin_start(bar, 12);
    gtk_widget_set_margin_end(bar, 12);
    GtkWidget *up = gtk_button_new_with_label("Up");
    g_signal_connect(up, "clicked", G_CALLBACK(on_up), files);
    files->path = gtk_entry_new();
    gtk_entry_set_placeholder_text(GTK_ENTRY(files->path), "/home/unify");
    gtk_widget_set_hexpand(files->path, TRUE);
    g_signal_connect(files->path, "activate", G_CALLBACK(on_go), files);
    GtkWidget *go = gtk_button_new_with_label("Go");
    g_signal_connect(go, "clicked", G_CALLBACK(on_go), files);
    gtk_box_append(GTK_BOX(bar), up);
    gtk_box_append(GTK_BOX(bar), files->path);
    gtk_box_append(GTK_BOX(bar), go);
    gtk_box_append(GTK_BOX(content), bar);

    files->list = gtk_list_box_new();
    gtk_list_box_set_selection_mode(GTK_LIST_BOX(files->list), GTK_SELECTION_SINGLE);
    gtk_widget_add_css_class(files->list, "boxed-list");
    gtk_widget_set_margin_start(files->list, 12);
    gtk_widget_set_margin_end(files->list, 12);
    g_signal_connect(files->list, "row-activated", G_CALLBACK(on_row_activated), files);
    gtk_box_append(GTK_BOX(content), unify_scrolled(files->list));

    files->status = gtk_label_new("");
    gtk_widget_add_css_class(files->status, "dim-label");
    gtk_label_set_xalign(GTK_LABEL(files->status), 0.0f);
    gtk_widget_set_margin_top(files->status, 6);
    gtk_widget_set_margin_bottom(files->status, 6);
    gtk_widget_set_margin_start(files->status, 12);
    gtk_box_append(GTK_BOX(content), files->status);

    gtk_window_set_child(GTK_WINDOW(window), content);

    const char *home = g_get_home_dir();
    navigate(files, home != NULL ? home : "/");

    g_object_set_data_full(G_OBJECT(window), "files", files, g_free);
    gtk_window_present(GTK_WINDOW(window));
}

int main(int argc, char **argv)
{
    GtkApplication *application =
        gtk_application_new("org.unifyos.Files", G_APPLICATION_DEFAULT_FLAGS);
    g_signal_connect(application, "activate", G_CALLBACK(on_activate), NULL);
    int status = g_application_run(G_APPLICATION(application), argc, argv);
    g_object_unref(application);
    return status;
}
