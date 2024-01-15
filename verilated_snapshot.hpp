#ifndef __VERILATOR_SNAPSHOT_HPP__
#define __VERILATOR_SNAPSHOT_HPP__

#include <verilated_save.h>
#include <cstdint>
#include <cstddef>

#define VL_SNAPSHOT_ERR(format)                                                \
    fprintf(stderr, format);                                                   \
    exit(1);

#define VL_SNAPSHOT_NULL_ERR(ptr, format)                                      \
    if (!ptr) {                                                                \
        VL_SNAPSHOT_ERR(format);                                               \
    }

class VerilatedSnapshot final : public VerilatedSerialize {
  private:
    uint8_t *m_endp;
	bool owns_buffer;

    static constexpr size_t initialSize() { return 256 * 1024; };
    static constexpr size_t bufferInsertSize() { return 16 * 1024; }
    static constexpr size_t bufferGrow(size_t previousSize) {
        return previousSize * 2;
    };

	/// Grows the buffer to the `needed_size`.
	///
	/// After calling this, the buffer is ensured to be larger than
	/// `needed_size` bytes. This does nothing if `needed_size` <
	/// `currentBufferSize()`.
    void growBuffer(size_t needed_size) {
        if (needed_size < currentBufferSize())
            return;

        size_t grown_size = currentBufferSize();
        while (grown_size < needed_size)
            grown_size = bufferGrow(grown_size);

        size_t offset = m_cp - m_bufp;

        m_bufp = (uint8_t *)realloc(m_bufp, grown_size);
        VL_SNAPSHOT_NULL_ERR(
            m_bufp, "Failed to reallocate to grow VerilatedSnapshot buffer\n");

        m_cp = m_bufp + offset;
        m_endp = m_bufp + grown_size;
    }

  public:
    VerilatedSnapshot() {
        size_t size = initialSize();

        m_bufp = (uint8_t *)malloc(size);
        VL_SNAPSHOT_NULL_ERR(
            m_bufp, "Failed to allocate initial VerilatedSnapshot buffer\n");
        m_cp = m_bufp;
        m_endp = m_bufp + size;

		owns_buffer = true;
    }
    VerilatedSnapshot(VerilatedSnapshot& cpy) {
		m_bufp = cpy.bufferStart();
		m_cp = cpy.bufferStart();
		m_endp = cpy.bufferEnd();

		owns_buffer = false;
    }

    ~VerilatedSnapshot() {
		if (owns_buffer) {
			free(m_bufp);
		}

		m_bufp = NULL;
	}

    void close() override { };
    void flush() override { growBuffer(currentBufferSize() + bufferInsertSize()); };

    size_t currentSnapshotSize() { return (size_t)(m_cp - bufferStart()); }
    size_t currentBufferSize() { return (size_t)(bufferEnd() - bufferStart()); }
    uint8_t *bufferStart() { return this->m_bufp; }
    uint8_t *bufferEnd() { return this->m_endp; }
};

class VerilatedSnapshotRestore final : public VerilatedDeserialize {
  private:
    VerilatedSnapshot *snapshot;

  public:
    VerilatedSnapshotRestore(VerilatedSnapshot *snapshot) : snapshot(snapshot) {
        delete m_bufp;
        reset();
    }
    ~VerilatedSnapshotRestore() {
		m_bufp = NULL;
	}

    void reset() {
        m_cp = snapshot->bufferStart();
        m_bufp = snapshot->bufferStart();
        m_endp = snapshot->bufferEnd();
    }

    void close() override{};
    void flush() override{};
    void fill() override{};
};

#endif // __VERILATOR_SNAPSHOT_HPP__
