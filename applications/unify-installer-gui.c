/* Unify Installer — graphical front end for the unify-installer backend.
 *
 * The application does not partition or format anything itself: it builds an
 * argument vector for the audited Python installer, which performs the safety
 * checks, partitioning and GRUB installation, and streams its log back here. */

#include "libunify.h"

#include <stdlib.h>
#include <string.h>

typedef struct {
    gchar **argv;
    gchar *password;
    GtkWidget *log;
    GtkWidget *button;
    GtkWidget *status;
    gchar *result;
} InstallJob;

typedef struct {
    GtkWidget *disks;
    GtkWidget *hostname;
    GtkWidget *username;
    GtkWidget *timezone;
    GtkWidget *locale;
    GtkWidget *keymap;
    GtkWidget *password;
    GtkWidget *dry_run;
    GtkWidget *log;
    GtkWidget *button;
    GtkWidget *status;
    GPtrArray *disk_paths;
} Installer;

static gboolean install_finished(gpointer data)
{
    InstallJob *job = data;
    if (job->result != NULL)
        unify_log_append(job->log, job->result);
    unify_log_append(job->log, "\n");
    gtk_widget_set_sensitive(job->button, TRUE);
    gtk_label_set_text(GTK_LABEL(job->status), "Installer finished. Review the log above.");
    g_strfreev(job->argv);
    g_free(job->password);
    g_free(job->result);
    g_free(job);
    return G_SOURCE_REMOVE;
}

static gpointer install_thread(gpointer data)
{
    InstallJob *job = data;
    GError *error = NULL;

    GSubprocess *process = g_subprocess_newv(
        (const gchar *const *)job->argv,
        G_SUBPROCESS_FLAGS_STDIN_PIPE | G_SUBPROCESS_FLAGS_STDOUT_PIPE |
            G_SUBPROCESS_FLAGS_STDERR_MERGE,
        &error);

    if (process == NULL) {
        job->result = g_strdup_printf("cannot start the installer: %s", error->message);
        g_error_free(error);
    } else {
        gchar *output = NULL;
        if (!g_subprocess_communicate_utf8(process,
                                           job->password != NULL ? job->password : "",
                                           NULL, &output, NULL, &error)) {
            job->result = g_strdup_printf("installer failed: %s", error->message);
            g_error_free(error);
            g_free(output);
        } else {
            job->result = output != NULL ? output : g_strdup("(no output)");
        }
        g_object_unref(process);
    }

    g_idle_add(install_finished, job);
    return NULL;
}

static char *entry_text(GtkWidget *entry)
{
    const char *text = gtk_editable_get_text(GTK_EDITABLE(entry));
    return g_strdup(text != NULL ? text : "");
}

static void on_install(GtkButton *button, gpointer data)
{
    (void)button;
    Installer *installer = data;

    guint selected = gtk_drop_down_get_selected(GTK_DROP_DOWN(installer->disks));
    if (selected == GTK_INVALID_LIST_POSITION || selected >= installer->disk_paths->len) {
        gtk_label_set_text(GTK_LABEL(installer->status), "Select a target disk first.");
        return;
    }
    const char *disk = g_ptr_array_index(installer->disk_paths, selected);
    if (disk == NULL || *disk == '\0') {
        gtk_label_set_text(GTK_LABEL(installer->status), "Select a target disk first.");
        return;
    }

    char *program = unify_tool("unify-installer");
    if (program == NULL) {
        gtk_label_set_text(GTK_LABEL(installer->status),
                           "The installer backend is not installed.");
        return;
    }

    GPtrArray *arguments = g_ptr_array_new_with_free_func(g_free);
    g_ptr_array_add(arguments, program);
    g_ptr_array_add(arguments, g_strdup("--disk"));
    g_ptr_array_add(arguments, g_strdup(disk));
    g_ptr_array_add(arguments, g_strdup("--hostname"));
    g_ptr_array_add(arguments, entry_text(installer->hostname));
    g_ptr_array_add(arguments, g_strdup("--username"));
    g_ptr_array_add(arguments, entry_text(installer->username));
    g_ptr_array_add(arguments, g_strdup("--timezone"));
    g_ptr_array_add(arguments, entry_text(installer->timezone));
    g_ptr_array_add(arguments, g_strdup("--locale"));
    g_ptr_array_add(arguments, entry_text(installer->locale));
    g_ptr_array_add(arguments, g_strdup("--keymap"));
    g_ptr_array_add(arguments, entry_text(installer->keymap));

    gboolean dry_run = gtk_switch_get_active(GTK_SWITCH(installer->dry_run));
    if (dry_run) {
        g_ptr_array_add(arguments, g_strdup("--dry-run"));
        g_ptr_array_add(arguments, NULL);
    } else {
        g_ptr_array_add(arguments, g_strdup("--yes"));
        g_ptr_array_add(arguments, g_strdup("--password-stdin"));
        g_ptr_array_add(arguments, NULL);
    }

    InstallJob *job = g_new0(InstallJob, 1);
    job->argv = (gchar **)g_ptr_array_free(arguments, FALSE);
    job->password = entry_text(installer->password);
    job->log = installer->log;
    job->button = installer->button;
    job->status = installer->status;

    gtk_widget_set_sensitive(installer->button, FALSE);
    gtk_label_set_text(GTK_LABEL(installer->status),
                       dry_run ? "Planning the installation…"
                               : "Installing. Do not power off the computer.");

    GThread *thread = g_thread_new("unify-install", install_thread, job);
    g_thread_unref(thread);
}

static void load_disks(Installer *installer)
{
    installer->disk_paths = g_ptr_array_new_with_free_func(g_free);
    GtkStringList *labels = gtk_string_list_new(NULL);

    char *program = unify_tool("unify-installer");
    char *json = NULL;
    if (program != NULL) {
        const char *argv[] = { program, "--list-disks", "--json", NULL };
        json = unify_run(argv);
    }

    if (json != NULL) {
        const char *cursor = strchr(json, '{');
        while (cursor != NULL) {
            const char *next = strstr(cursor, "},{");
            gsize length = next != NULL ? (gsize)(next - cursor + 1) : strlen(cursor);
            gchar *chunk = g_strndup(cursor, length);
            gchar *path = unify_json_string(chunk, "path");
            gchar *model = unify_json_string(chunk, "model");
            gchar *human = unify_json_string(chunk, "human_size");
            if (path != NULL) {
                gchar *label = g_strdup_printf("%s — %s (%s)", path,
                                               model != NULL && *model != '\0' ? model : "disk",
                                               human != NULL ? human : "?");
                gtk_string_list_append(labels, label);
                g_ptr_array_add(installer->disk_paths, g_strdup(path));
                g_free(label);
            }
            g_free(path);
            g_free(model);
            g_free(human);
            g_free(chunk);
            cursor = next != NULL ? next + 3 : NULL;
        }
    }

    if (installer->disk_paths->len == 0)
        gtk_string_list_append(labels, "No disks found — start the installer as root");

    gtk_drop_down_set_model(GTK_DROP_DOWN(installer->disks), G_LIST_MODEL(labels));
    g_object_unref(labels);
    g_free(json);
    g_free(program);
}

static GtkWidget *form_row(GtkGrid *grid, int row, const char *label, GtkWidget *widget)
{
    GtkWidget *text = gtk_label_new(label);
    gtk_label_set_xalign(GTK_LABEL(text), 0.0f);
    gtk_grid_attach(grid, text, 0, row, 1, 1);
    gtk_widget_set_hexpand(widget, TRUE);
    gtk_grid_attach(grid, widget, 1, row, 1, 1);
    return widget;
}

static void on_activate(GtkApplication *application, gpointer data)
{
    (void)data;
    Installer *installer = g_new0(Installer, 1);

    GtkWidget *window = unify_window(application, "Install UnifyOS", 620, 720);
    GtkWidget *content = gtk_box_new(GTK_ORIENTATION_VERTICAL, 0);
    gtk_widget_set_margin_start(content, 14);
    gtk_widget_set_margin_end(content, 14);

    GtkWidget *title = gtk_label_new("Install UnifyOS");
    gtk_widget_add_css_class(title, "title-1");
    gtk_label_set_xalign(GTK_LABEL(title), 0.0f);
    gtk_widget_set_margin_top(title, 12);
    gtk_box_append(GTK_BOX(content), title);

    GtkWidget *warning = gtk_label_new(
        "Installing erases the selected disk. Everything on it is destroyed and "
        "cannot be recovered. Use the dry run first if you want to see the plan.");
    gtk_label_set_wrap(GTK_LABEL(warning), TRUE);
    gtk_label_set_xalign(GTK_LABEL(warning), 0.0f);
    gtk_widget_add_css_class(warning, "dim-label");
    gtk_box_append(GTK_BOX(content), warning);

    installer->disks = gtk_drop_down_new(NULL, NULL);
    installer->hostname = gtk_entry_new();
    gtk_editable_set_text(GTK_EDITABLE(installer->hostname), "unifyos");
    installer->username = gtk_entry_new();
    gtk_editable_set_text(GTK_EDITABLE(installer->username), "unify");
    installer->timezone = gtk_entry_new();
    gtk_editable_set_text(GTK_EDITABLE(installer->timezone), "UTC");
    installer->locale = gtk_entry_new();
    gtk_editable_set_text(GTK_EDITABLE(installer->locale), "en_US.UTF-8");
    installer->keymap = gtk_entry_new();
    gtk_editable_set_text(GTK_EDITABLE(installer->keymap), "us");
    installer->password = gtk_password_entry_new();
    gtk_password_entry_set_show_peek_icon(GTK_PASSWORD_ENTRY(installer->password), TRUE);

    GtkWidget *grid = gtk_grid_new();
    gtk_grid_set_row_spacing(GTK_GRID(grid), 8);
    gtk_grid_set_column_spacing(GTK_GRID(grid), 12);
    gtk_widget_set_margin_top(grid, 12);
    gtk_widget_set_margin_bottom(grid, 12);

    form_row(GTK_GRID(grid), 0, "Target disk", installer->disks);
    form_row(GTK_GRID(grid), 1, "Computer name", installer->hostname);
    form_row(GTK_GRID(grid), 2, "User name", installer->username);
    form_row(GTK_GRID(grid), 3, "Password", installer->password);
    form_row(GTK_GRID(grid), 4, "Timezone", installer->timezone);
    form_row(GTK_GRID(grid), 5, "Locale", installer->locale);
    form_row(GTK_GRID(grid), 6, "Keyboard", installer->keymap);
    gtk_box_append(GTK_BOX(content), grid);

    GtkWidget *dry_box = gtk_box_new(GTK_ORIENTATION_HORIZONTAL, 12);
    GtkWidget *dry_label = gtk_label_new("Dry run (plan only, nothing is written)");
    gtk_label_set_xalign(GTK_LABEL(dry_label), 0.0f);
    gtk_widget_set_hexpand(dry_label, TRUE);
    installer->dry_run = gtk_switch_new();
    gtk_switch_set_active(GTK_SWITCH(installer->dry_run), TRUE);
    gtk_box_append(GTK_BOX(dry_box), dry_label);
    gtk_box_append(GTK_BOX(dry_box), installer->dry_run);
    gtk_box_append(GTK_BOX(content), dry_box);

    installer->button = gtk_button_new_with_label("Start installation");
    gtk_widget_set_margin_top(installer->button, 10);
    g_signal_connect(installer->button, "clicked", G_CALLBACK(on_install), installer);
    gtk_box_append(GTK_BOX(content), installer->button);

    installer->status = gtk_label_new("Ready.");
    gtk_label_set_wrap(GTK_LABEL(installer->status), TRUE);
    gtk_label_set_xalign(GTK_LABEL(installer->status), 0.0f);
    gtk_widget_add_css_class(installer->status, "dim-label");
    gtk_widget_set_margin_top(installer->status, 6);
    gtk_box_append(GTK_BOX(content), installer->status);

    gtk_box_append(GTK_BOX(content), unify_heading("Installer log"));
    installer->log = unify_log_view();
    gtk_widget_set_size_request(installer->log, -1, 200);
    gtk_widget_set_margin_bottom(installer->log, 12);
    gtk_box_append(GTK_BOX(content), installer->log);

    gtk_window_set_child(GTK_WINDOW(window), unify_scrolled(content));
    load_disks(installer);

    g_object_set_data_full(G_OBJECT(window), "installer", installer, g_free);
    gtk_window_present(GTK_WINDOW(window));
}

int main(int argc, char **argv)
{
    GtkApplication *application =
        gtk_application_new("org.unifyos.Installer", G_APPLICATION_DEFAULT_FLAGS);
    g_signal_connect(application, "activate", G_CALLBACK(on_activate), NULL);
    int status = g_application_run(G_APPLICATION(application), argc, argv);
    g_object_unref(application);
    return status;
}
