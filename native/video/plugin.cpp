#include <QQuickItem>
#include <QQuickWindow>
#include <QSGSimpleTextureNode>
#include <QQmlExtensionPlugin>
#include <QLocalSocket>
#include <QJsonDocument>
#include <QJsonObject>
#include <QImage>
#include <QTimer>
#include <QtEndian>

// Local, pull-driven transport: at most one in-flight frame. Qt owns the GPU
// texture; no native-window embedding, image recompression, or temporary files.
class RemoteVideoItem : public QQuickItem {
    Q_OBJECT
    Q_PROPERTY(QString socketPath MEMBER socketPath NOTIFY sourceChanged)
    Q_PROPERTY(QString participant MEMBER participant NOTIFY sourceChanged)
    Q_PROPERTY(QString source MEMBER source NOTIFY sourceChanged)
    Q_PROPERTY(bool ready READ ready NOTIFY frameChanged)
    Q_PROPERTY(QString error READ error NOTIFY errorChanged)
    Q_PROPERTY(QSize frameSize READ frameSize NOTIFY frameChanged)
public:
    RemoteVideoItem() {
        setFlag(ItemHasContents, true);
        connect(this, &RemoteVideoItem::sourceChanged, this, &RemoteVideoItem::restart);
        connect(&socket, &QLocalSocket::connected, this, [this] {
            socket.write(QJsonDocument(QJsonObject{{"participant", participant}, {"source", source}}).toJson(QJsonDocument::Compact) + '\n');
            requestFrame();
        });
        connect(&socket, &QLocalSocket::readyRead, this, &RemoteVideoItem::readFrame);
        connect(&socket, &QLocalSocket::errorOccurred, this, [this](auto) {
            failure = "Stream connection lost"; emit errorChanged();
        });
        socket.setReadBufferSize(128 * 1024 * 1024 + 8);
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
        if (!node) { node = new QSGSimpleTextureNode; node->setOwnsTexture(true); }
        if (dirty || !node->texture()) {
            auto *texture = window()->createTextureFromImage(frame);
            auto *previous = node->texture();
            node->setOwnsTexture(false);
            node->setTexture(texture);
            delete previous;
            node->setOwnsTexture(true);
            node->setFiltering(QSGTexture::Linear);
            dirty = false;
        }
        const QSizeF size = frame.size().scaled(QSize(qMax(1, int(width())), qMax(1, int(height()))), Qt::KeepAspectRatio);
        node->setRect(QRectF((width() - size.width()) / 2, (height() - size.height()) / 2, size.width(), size.height()));
        return node;
    }
    void geometryChange(const QRectF &now, const QRectF &before) override {
        QQuickItem::geometryChange(now, before); update();
    }
private:
    QString socketPath, participant, source, failure;
    QLocalSocket socket;
    QByteArray buffer;
    QImage frame;
    bool dirty = false;
    quint32 frameWidth = 0, frameHeight = 0;
    quint64 expected = 0;
    int generation = 0;
    void restart() {
        if (!isComponentComplete()) return;
        ++generation;
        socket.abort(); buffer.clear(); expected = 0; frame = QImage(); failure.clear();
        emit frameChanged(); emit errorChanged(); update();
        if (!socketPath.isEmpty() && !participant.isEmpty() && !source.isEmpty()) socket.connectToServer(socketPath);
    }
    void requestFrame() {
        if (socket.state() != QLocalSocket::ConnectedState) return;
        const qreal scale = window() ? window()->devicePixelRatio() : 1;
        socket.write(QByteArray::number(qMax(1, int(width() * scale))) + ' ' + QByteArray::number(qMax(1, int(height() * scale))) + '\n');
    }
    void readFrame() {
        buffer.append(socket.readAll());
        if (!expected && buffer.size() >= 8) {
            frameWidth = qFromLittleEndian<quint32>(buffer.constData());
            frameHeight = qFromLittleEndian<quint32>(buffer.constData() + 4);
            expected = quint64(frameWidth) * frameHeight * 4;
            if (!frameWidth || !frameHeight || frameWidth > 16384 || frameHeight > 16384 || expected > 128 * 1024 * 1024) {
                socket.abort(); buffer.clear(); failure = "Invalid stream dimensions"; emit errorChanged(); return;
            }
        }
        if (!expected || quint64(buffer.size()) < expected + 8) return;
        frame = QImage(reinterpret_cast<const uchar *>(buffer.constData() + 8), int(frameWidth), int(frameHeight), QImage::Format_RGBA8888).copy();
        buffer.clear(); expected = 0; dirty = true;
        emit frameChanged(); update();
        const int current = generation;
        QTimer::singleShot(16, this, [this, current] { if (current == generation) requestFrame(); });
    }
};

class WispVideoPlugin : public QQmlExtensionPlugin {
    Q_OBJECT
    Q_PLUGIN_METADATA(IID QQmlExtensionInterface_iid)
public:
    void registerTypes(const char *uri) override { qmlRegisterType<RemoteVideoItem>(uri, 1, 0, "RemoteVideoItem"); }
};
#include "plugin.moc"
