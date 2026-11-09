/* Unify System Monitor — live CPU, memory, load and process view.
 *
 * Samples come from unify-ctl, which reads /proc directly. The refresh timer is
 * the only activity when the window is idle. */

#include "libunify.h"

#include <stdlib.h>

typedef struct {
    GtkWidget *cpu;
    GtkWidget *memory;
    GtkWidget *load;
    GtkWidget *uptime;
    GtkWidget *processes;
    guint timer;
} Monitor;

static char *ctl(const char *command, const char *argument)
{
    char *program = unify_tool("unify-ctl");
    if (program == NULL)
        return NULL;

    const char *argv[5] = { program, command, NULL, "--json", NULL };
    if (argument != NULL) {
        argv[2] = argument;
        argv[3] = "--json";
        argv[4] = NULL;
    }
    char *output = unify_run(argv);
    g_free(program);
    return output;
}

static void set_text(GtkWidget *label, const char *text)
{
    gtk_label_set_text(GTK_LABEL(label), text != NULL ? text : "unavailable");
}

static void refresh(Monitor *monitor)
{
    char *cpu_json = ctl("cpu", NULL);
    char *mem_json = ctl("mem", NULL);
    char *process_json = ctl("processes", NULL);
    char *info_json = ctl("sysinfo", NULL);
    char buffer[256];

    if (cpu_json != NULL) {
        double usage = unify_json_number(cpu_json, "usage_percent", -1);
        double cores = unify_json_number(cpu_json, "cores", 0);
        g_snprintf(buffer, sizeof buffer, "%.1f %% of %d cores", usage, (int)cores);
        set_text(monitor->cpu, buffer);
    } else {
        set_text(monitor->cpu, "unify-ctl not found");
    }

    if (mem_json != NULL) {
        double used_kb = unify_json_number(mem_json, "used_kb", 0);
        double total_kb = unify_json_number(mem_json, "total_kb", 0);
        double percent = unify_json_number(mem_json, "used_percent", 0);
        g_snprintf(buffer, sizeof buffer, "%.0f MiB of %.0f MiB (%.1f %%)",
                   used_kb / 1024.0, total_kb / 1024.0, percent);
        set_text(monitor->memory, buffer);
    } else {
        set_text(monitor->memory, "unavailable");
    }

    if (info_json != NULL) {
        double one = 0, five = 0, fifteen = 0, uptime = 0;
        const char *separator = strstr(info_json, "\"load\"");
        if (separator != NULL) {
            one = unify_json_number(separator, "one", 0);
            five = unify_json_number(separator, "five", 0);
            fifteen = unify_json_number(separator, "fifteen", 0);
        }
        uptime = unify_json_number(info_json, "uptime_seconds", 0);
        g_snprintf(buffer, sizeof buffer, "%.2f  %.2f  %.2f", one, five, fifteen);
        set_text(monitor->load, buffer);
        g_snprintf(buffer, sizeof buffer, "%d h %d min",
                   (int)(uptime / 3600), (int)((long)uptime % 3600 / 60));
        set_text(monitor->uptime, buffer);
    }

    while (gtk_list_box_get_row_at_index(GTK_LIST_BOX(monitor->processes), 0) != NULL) {
        GtkListBoxRow *row = gtk_list_box_get_row_at_index(GTK_LIST_BOX(monitor->processes), 0);
        gtk_list_box_remove(GTK_LIST_BOX(monitor->processes), GTK_WIDGET(row));
    }

    if (process_json != NULL) {
        const char *cursor = strchr(process_json, '{');
        int shown = 0;
        while (cursor != NULL && shown < 8) {
            const char *next = strstr(cursor, "},{");
            gsize length = next != NULL ? (gsize)(next - cursor + 1) : strlen(cursor);
            gchar *chunk = g_strndup(cursor, length);
            gchar *name = unify_json_string(chunk, "name");
            double rss = unify_json_number(chunk, "rss_kb", 0);
            double percent = unify_json_number(chunk, "cpu_percent", 0);
            if (name != NULL) {
                gchar *subtitle = g_strdup_printf("%.1f %% CPU · %.1f MiB",
                                                  percent, rss / 1024.0);
                gtk_list_box_append(GTK_LIST_BOX(monitor->processes),
                                    unify_row(name, subtitle));
                g_free(subtitle);
                g_free(name);
                shown++;
            }
            g_free(chunk);
            cursor = next != NULL ? next + 3 : NULL;
        }
    }

    g_free(cpu_json);
    g_free(mem_json);
    g_free(process_json);
    g_free(info_json);
}

static gboolean on_tick(gpointer data)
{
    refresh((Monitor *)data);
    return G_SOURCE_CONTINUE;
}

static void on_refresh_clicked(GtkButton *button, gpointer data)
{
    (void)button;
    refresh((Monitor *)data);
}

static void on_activate(GtkApplication *application, gpointer data)
{
    (void)data;
    Monitor *monitor = g_new0(Monitor, 1);

    GtkWidget *window = unify_window(application, "System Monitor", 460, 640);
    GtkWidget *box = gtk_box_new(GTK_ORIENTATION_VERTICAL, 0);

    GtkWidget *header = gtk_box_new(GTK_ORIENTATION_HORIZONTAL, 6);
    gtk_widget_set_margin_top(header, 10);
    gtk_widget_set_margin_bottom(header, 4);
    gtk_widget_set_margin_start(header, 12);
    gtk_widget_set_margin_end(header, 12);
    GtkWidget *refresh_button = gtk_button_new_with_label("Refresh");
    gtk_widget_set_hexpand(refresh_button, TRUE);
    gtk_widget_set_halign(refresh_button, GTK_ALIGN_END);
    g_signal_connect(refresh_button, "clicked", G_CALLBACK(on_refresh_clicked), monitor);
    gtk_box_append(GTK_BOX(header), refresh_button);
    gtk_box_append(GTK_BOX(box), header);

    monitor->cpu = gtk_label_new("…");
    monitor->memory = gtk_label_new("…");
    monitor->load = gtk_label_new("…");
    monitor->uptime = gtk_label_new("…");
    GtkWidget *labels = gtk_box_new(GTK_ORIENTATION_VERTICAL, 6);
    gtk_widget_set_margin_start(labels, 16);
    gtk_widget_set_margin_end(labels, 16);

    gtk_box_append(GTK_BOX(labels), unify_heading("CPU"));
    gtk_box_append(GTK_BOX(labels), monitor->cpu);
    gtk_box_append(GTK_BOX(labels), unify_heading("Memory"));
    gtk_box_append(GTK_BOX(labels), monitor->memory);
    gtk_box_append(GTK_BOX(labels), unify_heading("Load average"));
    gtk_box_append(GTK_BOX(labels), monitor->load);
    gtk_box_append(GTK_BOX(labels), unify_heading("Uptime"));
    gtk_box_append(GTK_BOX(labels), monitor->uptime);
    gtk_box_append(GTK_BOX(box), labels);

    gtk_box_append(GTK_BOX(box), unify_heading("Processes"));

    monitor->processes = gtk_list_box_new();
    gtk_list_box_set_selection_mode(GTK_LIST_BOX(monitor->processes), GTK_SELECTION_NONE);
    gtk_widget_add_css_class(monitor->processes, "boxed-list");
    gtk_widget_set_margin_start(monitor->processes, 12);
    gtk_widget_set_margin_end(monitor->processes, 12);
    gtk_widget_set_margin_bottom(monitor->processes, 12);
    gtk_box_append(GTK_BOX(box), unify_scrolled(monitor->processes));

    gtk_window_set_child(GTK_WINDOW(window), box);
    refresh(monitor);
    monitor->timer = g_timeout_add_seconds(2, on_tick, monitor);

    g_object_set_data_full(G_OBJECT(window), "monitor", monitor, g_free);
    gtk_window_present(GTK_WINDOW(window));
}

int main(int argc, char **argv)
{
    GtkApplication *application =
        gtk_application_new("org.unifyos.Monitor", G_APPLICATION_DEFAULT_FLAGS);
    g_signal_connect(application, "activate", G_CALLBACK(on_activate), NULL);
    int status = g_application_run(G_APPLICATION(application), argc, argv);
    g_object_unref(application);
    return status;
}
