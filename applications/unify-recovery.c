/* Unify Recovery — diagnostics and safe recovery actions.
 *
 * Everything here is read-only except the explicit restart/power-off buttons. */

#include "libunify.h"

#include <stdlib.h>

typedef struct {
    GtkWidget *log;
    GtkWidget *status;
} Recovery;

static void append(Recovery *recovery, const char *text)
{
    unify_log_append(recovery->log, text != NULL ? text : "(no output)");
    unify_log_append(recovery->log, "\n\n");
}

static void run_and_log(Recovery *recovery, const char *title, const char *const argv[])
{
    gchar *heading = g_strdup_printf("=== %s ===\n", title);
    unify_log_append(recovery->log, heading);
    g_free(heading);

    char *output = unify_run(argv);
    append(recovery, output != NULL ? output : "command failed or is not installed");
    g_free(output);
    gtk_label_set_text(GTK_LABEL(recovery->status), title);
}

static void on_failed_units(GtkButton *button, gpointer data)
{
    (void)button;
    Recovery *recovery = data;
    char *systemctl = unify_tool("systemctl");
    if (systemctl == NULL) {
        gtk_label_set_text(GTK_LABEL(recovery->status), "systemd is not running on this system");
        return;
    }
    const char *argv[] = { systemctl, "--failed", "--no-pager", "--plain", NULL };
    run_and_log(recovery, "Failed units", argv);
    g_free(systemctl);
}

static void on_disk_usage(GtkButton *button, gpointer data)
{
    (void)button;
    Recovery *recovery = data;
    char *df = unify_tool("df");
    if (df == NULL) {
        gtk_label_set_text(GTK_LABEL(recovery->status), "df is not installed");
        return;
    }
    const char *argv[] = { df, "-h", NULL };
    run_and_log(recovery, "Disk usage", argv);
    g_free(df);
}

static void on_kernel_log(GtkButton *button, gpointer data)
{
    (void)button;
    Recovery *recovery = data;
    char *journalctl = unify_tool("journalctl");
    if (journalctl == NULL) {
        gtk_label_set_text(GTK_LABEL(recovery->status), "journalctl is not installed");
        return;
    }
    const char *argv[] = { journalctl, "-p", "err", "-b", "--no-pager", "-n", "100", NULL };
    run_and_log(recovery, "Errors from this boot", argv);
    g_free(journalctl);
}

static void on_package_check(GtkButton *button, gpointer data)
{
    (void)button;
    Recovery *recovery = data;
    char *program = unify_tool("unify-pkg");
    if (program == NULL) {
        gtk_label_set_text(GTK_LABEL(recovery->status), "unify-pkg is not installed");
        return;
    }
    const char *argv[] = { program, "check", NULL };
    run_and_log(recovery, "Package database check", argv);
    g_free(program);
}

static void power_action(Recovery *recovery, const char *action)
{
    char *program = unify_tool("unify-ctl");
    if (program == NULL) {
        gtk_label_set_text(GTK_LABEL(recovery->status), "unify-ctl is not installed");
        return;
    }
    const char *argv[] = { program, "power", action, NULL };
    gboolean ok = unify_run_ok(argv);
    gtk_label_set_text(GTK_LABEL(recovery->status),
                       ok ? "Requested." : "The action failed; see the journal.");
    g_free(program);
}

static void on_reboot(GtkButton *button, gpointer data)
{
    (void)button;
    power_action((Recovery *)data, "reboot");
}

static void on_poweroff(GtkButton *button, gpointer data)
{
    (void)button;
    power_action((Recovery *)data, "poweroff");
}

static void on_activate(GtkApplication *application, gpointer data)
{
    (void)data;
    Recovery *recovery = g_new0(Recovery, 1);

    GtkWidget *window = unify_window(application, "Recovery", 640, 620);
    GtkWidget *content = gtk_box_new(GTK_ORIENTATION_VERTICAL, 0);

    GtkWidget *actions = gtk_box_new(GTK_ORIENTATION_VERTICAL, 6);
    gtk_widget_set_margin_top(actions, 12);
    gtk_widget_set_margin_bottom(actions, 6);
    gtk_widget_set_margin_start(actions, 12);
    gtk_widget_set_margin_end(actions, 12);

    GtkWidget *failed = gtk_button_new_with_label("Show failed units");
    GtkWidget *disk = gtk_button_new_with_label("Show disk usage");
    GtkWidget *errors = gtk_button_new_with_label("Show errors from this boot");
    GtkWidget *check = gtk_button_new_with_label("Check the package database");
    GtkWidget *reboot = gtk_button_new_with_label("Restart the computer");
    GtkWidget *poweroff = gtk_button_new_with_label("Shut down");

    g_signal_connect(failed, "clicked", G_CALLBACK(on_failed_units), recovery);
    g_signal_connect(disk, "clicked", G_CALLBACK(on_disk_usage), recovery);
    g_signal_connect(errors, "clicked", G_CALLBACK(on_kernel_log), recovery);
    g_signal_connect(check, "clicked", G_CALLBACK(on_package_check), recovery);
    g_signal_connect(reboot, "clicked", G_CALLBACK(on_reboot), recovery);
    g_signal_connect(poweroff, "clicked", G_CALLBACK(on_poweroff), recovery);

    gtk_box_append(GTK_BOX(actions), failed);
    gtk_box_append(GTK_BOX(actions), disk);
    gtk_box_append(GTK_BOX(actions), errors);
    gtk_box_append(GTK_BOX(actions), check);
    gtk_box_append(GTK_BOX(actions), reboot);
    gtk_box_append(GTK_BOX(actions), poweroff);
    gtk_box_append(GTK_BOX(content), actions);

    recovery->status = gtk_label_new("Diagnostics are read-only.");
    gtk_widget_add_css_class(recovery->status, "dim-label");
    gtk_label_set_xalign(GTK_LABEL(recovery->status), 0.0f);
    gtk_widget_set_margin_start(recovery->status, 12);
    gtk_widget_set_margin_bottom(recovery->status, 6);
    gtk_box_append(GTK_BOX(content), recovery->status);

    recovery->log = unify_log_view();
    gtk_widget_set_margin_start(recovery->log, 12);
    gtk_widget_set_margin_end(recovery->log, 12);
    gtk_widget_set_margin_bottom(recovery->log, 12);
    gtk_box_append(GTK_BOX(content), recovery->log);

    gtk_window_set_child(GTK_WINDOW(window), content);

    g_object_set_data_full(G_OBJECT(window), "recovery", recovery, g_free);
    gtk_window_present(GTK_WINDOW(window));
}

int main(int argc, char **argv)
{
    GtkApplication *application =
        gtk_application_new("org.unifyos.Recovery", G_APPLICATION_DEFAULT_FLAGS);
    g_signal_connect(application, "activate", G_CALLBACK(on_activate), NULL);
    int status = g_application_run(G_APPLICATION(application), argc, argv);
    g_object_unref(application);
    return status;
}
