#include <QQuickItem>
#include <QQuickWindow>
#include <QSGSimpleTextureNode>
#include <QQmlExtensionPlugin>
#include <QImage>
#include <QTimer>
#include <cstddef>
#include <type_traits>
#include "wisp-video/src/lib.rs.h"

// Keep the two-field repr(C) forwarding type in Rust in lockstep with Qt6.
struct RustPluginMetadata { const void *data; size_t size; };
static_assert(std::is_standard_layout_v<QPluginMetaData>);
static_assert(sizeof(QPluginMetaData) == sizeof(RustPluginMetadata));
static_assert(offsetof(QPluginMetaData, data) == offsetof(RustPluginMetadata, data));
static_assert(offsetof(QPluginMetaData, size) == offsetof(RustPluginMetadata, size));

// Qt-only glue: QML properties, the scene-graph texture, and plugin registration.
// Socket I/O, frame validation/storage, cancellation, and aspect-fit math are Rust.
class RemoteVideoItem : public QQuickItem {
    Q_OBJECT
    Q_PROPERTY(QString socketPath MEMBER socketPath NOTIFY sourceChanged)
    Q_PROPERTY(QString participant MEMBER participant NOTIFY sourceChanged)
    Q_PROPERTY(QString source MEMBER source NOTIFY sourceChanged)
    Q_PROPERTY(bool ready READ ready NOTIFY frameChanged)
    Q_PROPERTY(QString error READ error NOTIFY errorChanged)
    Q_PROPERTY(QSize frameSize READ frameSize NOTIFY frameChanged)
public:
    RemoteVideoItem() : receiver(wisp_video::new_receiver()) {
        setFlag(ItemHasContents, true);
        connect(this, &RemoteVideoItem::sourceChanged, this, &RemoteVideoItem::restart);
        connect(&timer, &QTimer::timeout, this, &RemoteVideoItem::poll);
        timer.start(16);
    }
    bool ready() const { return !frame.isNull(); }
    QString error() const { return failure; }
    QSize frameSize() const { return frame.size(); }
signals:
    void sourceChanged();
    void frameChanged();
    void errorChanged();
protected:
    void componentComplete() override { QQuickItem::componentComplete(); restart(); }
    QSGNode *updatePaintNode(QSGNode *old, UpdatePaintNodeData *) override {
        if (frame.isNull() || !window()) { delete old; return nullptr; }
        auto *node = static_cast<QSGSimpleTextureNode *>(old);
        if (!node) node = new QSGSimpleTextureNode;
        if (dirty || !node->texture()) {
            auto *texture = window()->createTextureFromImage(frame);
            auto *previous = node->texture();
            node->setOwnsTexture(false); node->setTexture(texture); delete previous;
            node->setOwnsTexture(true); node->setFiltering(QSGTexture::Linear); dirty = false;
        }
        const auto rect = wisp_video::fit_rect(frame.width(), frame.height(), width(), height());
        node->setRect(QRectF(rect.x, rect.y, rect.width, rect.height));
        return node;
    }
    void geometryChange(const QRectF &now, const QRectF &before) override {
        QQuickItem::geometryChange(now, before); update();
    }
private:
    rust::Box<wisp_video::VideoReceiver> receiver;
    QString socketPath, participant, source, failure;
    QTimer timer;
    QImage frame;
    bool dirty = false;
    void restart() {
        if (!isComponentComplete()) return;
        frame = QImage(); failure.clear(); emit frameChanged(); emit errorChanged(); update();
        receiver->start(socketPath.toStdString(), participant.toStdString(), source.toStdString());
    }
    void poll() {
        const qreal scale = window() ? window()->devicePixelRatio() : 1;
        receiver->viewport(qBound(1, int(width() * scale), 16384), qBound(1, int(height() * scale), 16384));
        auto next = receiver->poll();
        if (!next.changed) return;
        failure = QString::fromUtf8(next.error.data(), qsizetype(next.error.size())); emit errorChanged();
        if (next.pixels.empty()) return;
        frame = QImage(next.pixels.data(), int(next.width), int(next.height), QImage::Format_RGBA8888).copy();
        dirty = true; emit frameChanged(); update();
    }
};

class WispVideoPlugin : public QQmlExtensionPlugin {
    Q_OBJECT
    Q_PLUGIN_METADATA(IID QQmlExtensionInterface_iid)
public:
    void registerTypes(const char *uri) override { qmlRegisterType<RemoteVideoItem>(uri, 1, 0, "RemoteVideoItem"); }
};
// Rust exports the two loader entry points, forwarding to these Qt-generated ones.
#define qt_plugin_instance wisp_qt_plugin_instance
#define qt_plugin_query_metadata_v2 wisp_qt_plugin_query_metadata_v2
#include "qt.moc"
